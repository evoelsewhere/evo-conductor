//! Syncs the models.dev catalog into Conductor's own rate table.
//!
//! Conductor prices telemetry itself so the whole fleet shares one rate table
//! and history can be repriced; see `conductor_domain::pricing` for the
//! arithmetic. This module is only responsible for turning the upstream
//! document into append-only rate rows.
//!
//! Long-context bands (`cost.tiers` and the older `cost.context_over_200k`)
//! and per-service-tier rates (`experimental.modes`) are read here and
//! resolved per call by `conductor_domain::price_model_call_tiered`, matching
//! EvoFlux. Both are load-bearing for accuracy, not refinements: a band
//! replaces the rate for the whole request, so a 300K-token Sonnet turn
//! priced at the headline rate reads about a third low, and a fast lane bills
//! 2-2.5x.
//!
//! Remaining fidelity gaps, both of which read as a rate difference rather
//! than a missing rate — `unpriced_reason` cannot express either, which is
//! why the client's `estimated_cost_usd_micros` is retained for comparison:
//!
//! - **Audio tokens.** EvoFlux prices them; models.dev publishes them under
//!   keys this module does not read, and telemetry carries no audio counter
//!   to price against.
//! - **Client-implemented lanes.** EvoFlux unions the catalog's modes with
//!   tiers its own integrations implement (Codex's fast lane belongs to a
//!   ChatGPT subscription, so no catalog lists it). Conductor only knows what
//!   models.dev publishes, so a call on such a lane prices at the ordinary
//!   rate.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use conductor_domain::{
    cache_read_savings_usd_micros, normalize_model_key, price_model_call_tiered,
    rate_from_usd_per_million, ModelPricing, ModelRates, PricedCost, RateTier, ServiceTierRates,
    TokenUsage, UnpricedReason,
};
use conductor_storage::repos::{ModelPriceRow, PriceCatalogSnapshot};
use conductor_storage::{Db, StorageError};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
/// The document is a few megabytes; this only guards against a redirect to
/// something unbounded.
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
pub const SOURCE_MODELS_DEV: &str = "models_dev";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub version: String,
    pub model_count: u32,
    pub priced_model_count: u32,
    pub changed_model_count: u32,
    /// True when the fetched document was byte-identical to the last sync, so
    /// no parsing or writing happened.
    pub unchanged: bool,
}

const PROVIDER_ALIASES: &[(&str, &str)] = &[
    ("googlegenai", "google"),
    ("kimi", "kimi-for-coding"),
    ("copilot", "github-copilot"),
];

pub fn resolve_provider_alias(provider: &str) -> &str {
    let normalized = provider.trim().to_ascii_lowercase();
    PROVIDER_ALIASES
        .iter()
        .find(|(alias, _)| *alias == normalized)
        .map(|(_, canonical)| *canonical)
        .unwrap_or(provider)
}

/// The newest rate per model, held in memory so pricing a telemetry batch
/// costs no queries.
///
/// `newest_effective_from` is the gate that keeps this correct: it is the most
/// recent `effective_from` in the snapshot, so an event reported before it may
/// predate a price change and must be resolved against the database instead. A
/// client that was offline for a week drains a backlog of such events.
#[derive(Debug, Default)]
pub struct RateTable {
    pub catalog_version: Option<String>,
    pub newest_effective_from: Option<DateTime<Utc>>,
    rates: HashMap<(String, String), ModelPricing>,
}

impl RateTable {
    pub fn is_loaded(&self) -> bool {
        self.catalog_version.is_some()
    }

    pub fn len(&self) -> usize {
        self.rates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rates.is_empty()
    }

    fn get(&self, provider: &str, model: &str) -> Option<&ModelPricing> {
        let provider = resolve_provider_alias(provider);
        self.rates
            .get(&(normalize_model_key(provider), normalize_model_key(model)))
    }

    /// The current **headline** rate for one model, for a caller outside this
    /// module that only needs a snapshot lookup rather than the ingest-time
    /// resolution `price_event` does.
    ///
    /// Deliberately the base rate, with no long-context band applied. Its one
    /// caller splits an already-summed period total into components, and a
    /// band is a property of each individual request's prompt size, not of a
    /// period's token sum — resolving one here would pick a band from an
    /// aggregate that no single call necessarily reached. The split is a
    /// ratio over an authoritative total, so the headline rate is the right
    /// and safe proportion to use.
    pub fn current_rate_for(&self, provider: &str, model: &str) -> Option<ModelRates> {
        self.get(provider, model).map(|pricing| pricing.base)
    }
}

#[cfg(test)]
impl RateTable {
    /// Build a rate table directly from entries, without a synced catalog.
    /// `#[cfg(test)]` still lets other modules construct one during
    /// `cargo test`, since cfg(test) applies crate-wide.
    pub fn for_test(
        entries: impl IntoIterator<Item = (&'static str, &'static str, ModelRates)>,
    ) -> Self {
        Self::for_test_pricing(
            entries
                .into_iter()
                .map(|(provider, model, rates)| (provider, model, ModelPricing::flat(rates))),
        )
    }

    /// As [`Self::for_test`], but with bands and lanes.
    pub fn for_test_pricing(
        entries: impl IntoIterator<Item = (&'static str, &'static str, ModelPricing)>,
    ) -> Self {
        Self {
            rates: entries
                .into_iter()
                .map(|(provider, model, pricing)| {
                    (
                        (normalize_model_key(provider), normalize_model_key(model)),
                        pricing,
                    )
                })
                .collect(),
            ..Self::default()
        }
    }
}

/// Process-wide handle to the current [`RateTable`], swapped wholesale after
/// each successful sync. Readers clone an `Arc`, so pricing never blocks on a
/// sync and never sees a half-updated table.
#[derive(Clone, Default)]
pub struct SharedRateTable(Arc<RwLock<Arc<RateTable>>>);

impl SharedRateTable {
    pub fn snapshot(&self) -> Arc<RateTable> {
        self.0.read().expect("rate table poisoned").clone()
    }

    pub fn replace(&self, table: RateTable) {
        *self.0.write().expect("rate table poisoned") = Arc::new(table);
    }

    /// Rebuild from the database. Called at startup and after a sync.
    pub async fn reload(&self, db: &Db) -> Result<usize, StorageError> {
        let prices = db.model_prices();
        let catalog = prices.latest_catalog().await?;
        let rows = prices.current_rate_table_with_effective_from().await?;
        let newest_effective_from = rows.iter().map(|(_, effective)| *effective).max();
        let rates = rows
            .into_iter()
            .map(|(row, _)| ((row.provider, row.model), row.pricing))
            .collect::<HashMap<_, _>>();
        let loaded = rates.len();
        self.replace(RateTable {
            catalog_version: catalog.map(|catalog| catalog.version),
            newest_effective_from,
            rates,
        });
        Ok(loaded)
    }
}

/// Price one model call, preferring the in-memory table.
///
/// Falls back to a database lookup only for an event reported before the
/// newest known rate change, where the current rate may not be the one that
/// was in force. Everything else is served from memory.
pub async fn price_event(
    db: &Db,
    table: &SharedRateTable,
    provider: Option<&str>,
    model: Option<&str>,
    usage: TokenUsage,
    service_tier: Option<&str>,
    reported_at: DateTime<Utc>,
) -> Result<(PricedCost, Option<String>, Option<i64>), StorageError> {
    let snapshot = table.snapshot();
    if !snapshot.is_loaded() {
        return Ok((
            PricedCost::Unpriced {
                reason: UnpricedReason::NoCatalog,
            },
            None,
            None,
        ));
    }
    let (Some(provider), Some(model)) = (provider, model) else {
        return Ok((
            PricedCost::Unpriced {
                reason: UnpricedReason::ModelUnknown,
            },
            None,
            None,
        ));
    };

    let provider = resolve_provider_alias(provider);

    let predates_snapshot = snapshot
        .newest_effective_from
        .is_some_and(|newest| reported_at < newest);
    // A backdated event is priced from the historical row, so the catalog
    // recorded against it has to be that row's own — not whichever catalog
    // happens to be current, which never priced this event.
    let resolved = if predates_snapshot {
        db.model_prices()
            .pricing_at(provider, model, reported_at)
            .await?
            .map(|found| (found.pricing, Some(found.catalog_version)))
    } else {
        snapshot
            .get(provider, model)
            .map(|pricing| (pricing.clone(), snapshot.catalog_version.clone()))
    };

    let Some((pricing, catalog_version)) = resolved else {
        return Ok((
            PricedCost::Unpriced {
                reason: UnpricedReason::ModelUnknown,
            },
            None,
            None,
        ));
    };
    let cost = price_model_call_tiered(&pricing, usage, service_tier);
    let cache_savings = cost
        .cost_micros()
        .and(cache_read_savings_usd_micros(&pricing, usage, service_tier));
    let version = cost.cost_micros().and(catalog_version);
    Ok((cost, version, cache_savings))
}

#[derive(Debug, Deserialize)]
struct UpstreamProvider {
    #[serde(default)]
    models: HashMap<String, UpstreamModel>,
}

#[derive(Debug, Deserialize)]
struct UpstreamModel {
    #[serde(default)]
    cost: Option<UpstreamCost>,
    #[serde(default)]
    experimental: Option<UpstreamExperimental>,
}

/// models.dev quotes every rate as USD per million tokens. Unknown keys are
/// ignored rather than rejected so an upstream addition cannot break a sync.
#[derive(Debug, Default, Deserialize)]
struct UpstreamRates {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
    reasoning: Option<f64>,
}

/// The headline rates plus the long-context bands that replace them.
///
/// models.dev states a long-context surcharge two ways: the newer `tiers`
/// list, and the older `context_over_200k` object. Both mean the same thing,
/// so both are read and normalized into one shape — a row that has moved to
/// `tiers` must not be read twice, which is why the older spelling only
/// contributes a band no `tiers` entry already covers.
#[derive(Debug, Deserialize)]
struct UpstreamCost {
    #[serde(flatten)]
    rates: UpstreamRates,
    #[serde(default)]
    tiers: Vec<UpstreamTier>,
    #[serde(default)]
    context_over_200k: Option<UpstreamRates>,
}

#[derive(Debug, Deserialize)]
struct UpstreamTier {
    #[serde(flatten)]
    rates: UpstreamRates,
    #[serde(default)]
    tier: Option<UpstreamTierThreshold>,
}

#[derive(Debug, Deserialize)]
struct UpstreamTierThreshold {
    #[serde(default)]
    size: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct UpstreamExperimental {
    /// Alternate service tiers for the same model — OpenAI's priority lane,
    /// Anthropic's fast-mode beta. Only the rates matter here: the wire patch
    /// that selects a lane is the client's business, Conductor only has to
    /// price a lane the client says it used.
    #[serde(default)]
    modes: HashMap<String, UpstreamMode>,
}

#[derive(Debug, Deserialize)]
struct UpstreamMode {
    #[serde(default)]
    cost: Option<UpstreamRates>,
}

/// Fetch models.dev and apply it, skipping all work when the document has not
/// changed since the last sync.
pub async fn sync_from_models_dev(db: &Db, url: &str) -> anyhow::Result<SyncOutcome> {
    let client = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build()?;
    let response = client.get(url).send().await?.error_for_status()?;
    let payload = response.bytes().await?;
    if payload.len() > MAX_PAYLOAD_BYTES {
        anyhow::bail!("models.dev payload exceeded {MAX_PAYLOAD_BYTES} bytes");
    }
    apply_catalog(db, &payload, url, SOURCE_MODELS_DEV).await
}

/// Apply an already-fetched catalog document. Split out from the fetch so a
/// sync can be tested, and an operator-supplied snapshot applied, without
/// network access.
pub async fn apply_catalog(
    db: &Db,
    payload: &[u8],
    source_url: &str,
    source: &str,
) -> anyhow::Result<SyncOutcome> {
    let version = catalog_version(payload);
    let prices = db.model_prices();

    if let Some(latest) = prices.latest_catalog().await? {
        if latest.version == version {
            return Ok(SyncOutcome {
                version,
                model_count: latest.model_count,
                priced_model_count: latest.priced_model_count,
                changed_model_count: 0,
                unchanged: true,
            });
        }
    }

    let parsed = parse_catalog(payload)?;
    let model_count = parsed.len() as u32;
    let priced_model_count = parsed
        .iter()
        .filter(|row| !row.pricing.is_empty())
        .count()
        .try_into()
        .unwrap_or(u32::MAX);

    let existing: HashMap<(String, String), ModelPricing> = prices
        .current_rate_table()
        .await?
        .into_iter()
        .map(|row| ((row.provider, row.model), row.pricing))
        .collect();

    // Only a real rate change earns a new row: otherwise every upstream edit
    // to a description would add a redundant effective_from and make the
    // history unreadable.
    let changed: Vec<ModelPriceRow> = parsed
        .into_iter()
        .filter(|row| {
            existing
                .get(&(row.provider.clone(), row.model.clone()))
                .is_none_or(|current| *current != row.pricing)
        })
        .collect();
    let changed_model_count = changed.len() as u32;

    let effective_from = chrono::Utc::now();
    // The catalog row must exist before any price references it.
    prices
        .record_catalog(&PriceCatalogSnapshot {
            version: version.clone(),
            source: source.to_string(),
            source_url: Some(source_url.to_string()),
            fetched_at: effective_from,
            model_count,
            priced_model_count,
            changed_model_count,
        })
        .await?;
    prices
        .insert_prices(&version, effective_from, &changed)
        .await?;

    Ok(SyncOutcome {
        version,
        model_count,
        priced_model_count,
        changed_model_count,
        unchanged: false,
    })
}

/// Identify a catalog by its content, so an unchanged upstream document is
/// recognised without comparing every model.
pub fn catalog_version(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    hex::encode(digest)
}

fn parse_catalog(payload: &[u8]) -> anyhow::Result<Vec<ModelPriceRow>> {
    let providers: HashMap<String, UpstreamProvider> = serde_json::from_slice(payload)?;
    let mut rows = Vec::new();
    for (provider_id, provider) in providers {
        let provider_key = normalize_model_key(&provider_id);
        if provider_key.is_empty() {
            continue;
        }
        for (model_id, model) in provider.models {
            let model_key = normalize_model_key(&model_id);
            if model_key.is_empty() {
                continue;
            }
            rows.push(ModelPriceRow {
                provider: provider_key.clone(),
                model: model_key,
                pricing: pricing_from_upstream(model.cost, model.experimental),
            });
        }
    }
    rows.sort_by(|left, right| (&left.provider, &left.model).cmp(&(&right.provider, &right.model)));
    Ok(rows)
}

fn rates_from_upstream(cost: &UpstreamRates) -> ModelRates {
    ModelRates {
        input: cost.input.and_then(rate_from_usd_per_million),
        output: cost.output.and_then(rate_from_usd_per_million),
        cache_read: cost.cache_read.and_then(rate_from_usd_per_million),
        cache_write: cost.cache_write.and_then(rate_from_usd_per_million),
        reasoning: cost.reasoning.and_then(rate_from_usd_per_million),
    }
}

fn pricing_from_upstream(
    cost: Option<UpstreamCost>,
    experimental: Option<UpstreamExperimental>,
) -> ModelPricing {
    let service_tiers = experimental
        .map(|experimental| {
            experimental
                .modes
                .into_iter()
                .filter_map(|(name, mode)| {
                    let rates = rates_from_upstream(&mode.cost?);
                    // A lane that publishes no rate of its own bills at the
                    // model's ordinary price; storing it would only add an
                    // overlay that changes nothing.
                    (!rates.is_empty()).then(|| ServiceTierRates {
                        tier: normalize_model_key(&name),
                        rates,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let Some(cost) = cost else {
        return ModelPricing {
            base: ModelRates::default(),
            tiers: Vec::new(),
            service_tiers,
        }
        .sorted();
    };

    let mut tiers: Vec<RateTier> = cost
        .tiers
        .iter()
        .filter_map(|tier| {
            let above_tokens =
                tier.tier
                    .as_ref()
                    .and_then(|threshold| threshold.size)
                    .filter(|size| size.is_finite() && *size >= 0.0)? as i64;
            let rates = rates_from_upstream(&tier.rates);
            (!rates.is_empty()).then_some(RateTier {
                above_tokens,
                rates,
            })
        })
        .collect();

    // The legacy spelling is a fallback only: a row carrying both states the
    // same surcharge twice, and adding it again would leave two bands at the
    // same threshold whose winner depended on iteration order.
    if let Some(legacy) = cost.context_over_200k.as_ref() {
        let rates = rates_from_upstream(legacy);
        if !rates.is_empty() && !tiers.iter().any(|tier| tier.above_tokens == 200_000) {
            tiers.push(RateTier {
                above_tokens: 200_000,
                rates,
            });
        }
    }

    ModelPricing {
        base: rates_from_upstream(&cost.rates),
        tiers,
        service_tiers,
    }
    .sorted()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = br#"{
        "anthropic": {
            "id": "anthropic",
            "name": "Anthropic",
            "models": {
                "claude-sonnet-4": {
                    "id": "claude-sonnet-4",
                    "name": "Claude Sonnet 4",
                    "cost": {"input": 3, "output": 15, "cache_read": 0.3, "cache_write": 3.75}
                },
                "claude-haiku-4": {
                    "id": "claude-haiku-4",
                    "cost": {"input": 0.8, "output": 4}
                }
            }
        },
        "OpenAI": {
            "id": "openai",
            "models": {
                "GPT-5": {"id": "gpt-5", "cost": {"input": 1.25, "output": 10, "reasoning": 10}},
                "local-only": {"id": "local-only"}
            }
        }
    }"#;

    #[test]
    fn parses_every_provider_and_model_with_normalized_keys() {
        let rows = parse_catalog(SAMPLE).expect("parse");
        assert_eq!(rows.len(), 4);
        let keys: Vec<_> = rows
            .iter()
            .map(|row| (row.provider.as_str(), row.model.as_str()))
            .collect();
        assert_eq!(
            keys,
            vec![
                ("anthropic", "claude-haiku-4"),
                ("anthropic", "claude-sonnet-4"),
                ("openai", "gpt-5"),
                ("openai", "local-only"),
            ]
        );
    }

    #[test]
    fn converts_usd_per_million_into_micro_rates() {
        let rows = parse_catalog(SAMPLE).expect("parse");
        let sonnet = rows
            .iter()
            .find(|row| row.model == "claude-sonnet-4")
            .expect("sonnet");
        assert_eq!(sonnet.pricing.base.input, Some(3_000_000));
        assert_eq!(sonnet.pricing.base.output, Some(15_000_000));
        assert_eq!(sonnet.pricing.base.cache_read, Some(300_000));
        assert_eq!(sonnet.pricing.base.cache_write, Some(3_750_000));
        assert_eq!(sonnet.pricing.base.reasoning, None);
    }

    /// A model with no `cost` object must land as fully unpriced rather than
    /// being dropped: the row records that Conductor knows the model and
    /// upstream publishes no price.
    #[test]
    fn a_model_without_a_cost_object_is_kept_but_unpriced() {
        let rows = parse_catalog(SAMPLE).expect("parse");
        let local = rows
            .iter()
            .find(|row| row.model == "local-only")
            .expect("local-only");
        assert!(local.pricing.is_empty());
    }

    /// Unknown upstream keys must not fail a sync — models.dev adds fields.
    #[test]
    fn unknown_upstream_fields_are_ignored() {
        let payload = br#"{
            "acme": {
                "id": "acme", "brand_new_field": 42,
                "models": {
                    "m1": {"id": "m1", "another_new_field": "x",
                           "cost": {"input": 1, "output": 2, "input_audio": 9}}
                }
            }
        }"#;
        let rows = parse_catalog(payload).expect("parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pricing.base.input, Some(1_000_000));
    }

    /// The newer `cost.tiers` spelling, with the threshold under `tier.size`.
    #[test]
    fn reads_the_tiers_list_spelling_of_a_long_context_band() {
        let payload = br#"{
            "anthropic": {
                "id": "anthropic",
                "models": {
                    "claude-sonnet-4": {
                        "id": "claude-sonnet-4",
                        "cost": {
                            "input": 3, "output": 15,
                            "tiers": [
                                {"tier": {"size": 200000}, "input": 6, "output": 22.5}
                            ]
                        }
                    }
                }
            }
        }"#;
        let rows = parse_catalog(payload).expect("parse");
        let pricing = &rows[0].pricing;
        assert_eq!(pricing.base.input, Some(3_000_000));
        assert_eq!(pricing.tiers.len(), 1);
        assert_eq!(pricing.tiers[0].above_tokens, 200_000);
        assert_eq!(pricing.tiers[0].rates.input, Some(6_000_000));
        assert_eq!(pricing.tiers[0].rates.output, Some(22_500_000));
    }

    /// The older `context_over_200k` object means the same thing and must be
    /// read for rows that have not moved to `tiers`.
    #[test]
    fn reads_the_legacy_context_over_200k_spelling() {
        let payload = br#"{
            "anthropic": {
                "id": "anthropic",
                "models": {
                    "claude-sonnet-4": {
                        "id": "claude-sonnet-4",
                        "cost": {
                            "input": 3, "output": 15,
                            "context_over_200k": {"input": 6, "output": 22.5}
                        }
                    }
                }
            }
        }"#;
        let rows = parse_catalog(payload).expect("parse");
        let pricing = &rows[0].pricing;
        assert_eq!(pricing.tiers.len(), 1);
        assert_eq!(pricing.tiers[0].above_tokens, 200_000);
        assert_eq!(pricing.tiers[0].rates.input, Some(6_000_000));
    }

    /// A row carrying both spellings states one surcharge twice. Two bands at
    /// the same threshold would make the winner depend on iteration order.
    #[test]
    fn a_row_with_both_spellings_yields_one_band() {
        let payload = br#"{
            "anthropic": {
                "id": "anthropic",
                "models": {
                    "claude-sonnet-4": {
                        "id": "claude-sonnet-4",
                        "cost": {
                            "input": 3, "output": 15,
                            "tiers": [{"tier": {"size": 200000}, "input": 6}],
                            "context_over_200k": {"input": 6}
                        }
                    }
                }
            }
        }"#;
        let rows = parse_catalog(payload).expect("parse");
        assert_eq!(rows[0].pricing.tiers.len(), 1);
    }

    /// A band with no threshold cannot be applied to anything, and one with
    /// no rates changes nothing. Both must be dropped rather than stored as
    /// a band that silently never fires or fires with empty rates.
    #[test]
    fn unusable_bands_are_dropped() {
        let payload = br#"{
            "acme": {
                "id": "acme",
                "models": {
                    "m1": {
                        "id": "m1",
                        "cost": {
                            "input": 1, "output": 2,
                            "tiers": [
                                {"input": 9},
                                {"tier": {"size": 100000}},
                                {"tier": {"size": 300000}, "input": 4}
                            ]
                        }
                    }
                }
            }
        }"#;
        let rows = parse_catalog(payload).expect("parse");
        let tiers = &rows[0].pricing.tiers;
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0].above_tokens, 300_000);
    }

    /// Alternate service tiers live under `experimental.modes`, keyed by the
    /// name the client reports back in `service_tier`.
    #[test]
    fn reads_service_tier_rates_from_experimental_modes() {
        let payload = br#"{
            "openai": {
                "id": "openai",
                "models": {
                    "gpt-5": {
                        "id": "gpt-5",
                        "cost": {"input": 1.25, "output": 10},
                        "experimental": {
                            "modes": {
                                "Priority": {
                                    "cost": {"input": 2.5, "output": 20},
                                    "provider": {"body": {"service_tier": "priority"}}
                                },
                                "no-rates": {"provider": {"body": {"x": 1}}}
                            }
                        }
                    }
                }
            }
        }"#;
        let rows = parse_catalog(payload).expect("parse");
        let lanes = &rows[0].pricing.service_tiers;
        assert_eq!(lanes.len(), 1, "a lane with no rates of its own is dropped");
        assert_eq!(lanes[0].tier, "priority", "normalized for lookup");
        assert_eq!(lanes[0].rates.input, Some(2_500_000));
    }

    /// Bands and lanes must survive a round trip through the rate table the
    /// same way the headline rates do, or a synced band would be lost
    /// between the parse and the first event priced against it.
    #[test]
    fn a_banded_model_prices_per_call_through_the_rate_table() {
        let rows = parse_catalog(
            br#"{
            "anthropic": {
                "id": "anthropic",
                "models": {
                    "claude-sonnet-4": {
                        "id": "claude-sonnet-4",
                        "cost": {
                            "input": 3, "output": 15,
                            "tiers": [{"tier": {"size": 200000}, "input": 6}]
                        }
                    }
                }
            }
        }"#,
        )
        .expect("parse");
        let pricing = rows[0].pricing.clone();

        let small = price_model_call_tiered(
            &pricing,
            TokenUsage {
                tokens_in: 10_000,
                ..TokenUsage::default()
            },
            None,
        );
        let large = price_model_call_tiered(
            &pricing,
            TokenUsage {
                tokens_in: 300_000,
                ..TokenUsage::default()
            },
            None,
        );
        assert_eq!(small.cost_micros(), Some(30_000));
        assert_eq!(large.cost_micros(), Some(1_800_000));
    }

    #[test]
    fn version_is_content_addressed() {
        let first = catalog_version(SAMPLE);
        assert_eq!(first, catalog_version(SAMPLE));
        assert_ne!(first, catalog_version(b"{}"));
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn a_malformed_document_is_an_error_not_an_empty_catalog() {
        // An empty catalog would silently mark every model unpriced.
        assert!(parse_catalog(b"not json").is_err());
        assert!(parse_catalog(b"[]").is_err());
    }
}
