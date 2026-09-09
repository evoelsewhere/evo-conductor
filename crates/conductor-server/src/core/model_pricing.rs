//! Server-side model-call pricing catalog.
//!
//! Mirrors the evoflux client's own catalog client
//! (`app/agent/providers/model_registry.py`): fetches and caches
//! `https://models.dev/api.json` independently, so Conductor can price a
//! `model_call` event on its own when the reporting client did not send a
//! cost. The pure rate-resolution and cost math live in
//! `conductor_domain::pricing` — this module only owns the I/O (fetch,
//! disk cache, catalog lookup).
//!
//! Conductor only ever *fills a gap*: when a client already priced a call,
//! that value is left untouched (see the ingest handler in
//! `http::routes::telemetry`). This module is consulted only for calls the
//! client could not price itself.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use conductor_domain::{
    estimate_cost_usd, resolve_rates, CostTier, ModelCostRates, ModelPricing,
    ModelPricingCatalogEntry, ModelPricingCatalogSnapshot,
};
use serde_json::{Map, Value};

use crate::core::constants::pricing::{
    MODELS_DEV_CACHE_TTL_SECONDS, MODELS_DEV_FETCH_TIMEOUT_SECONDS, MODELS_DEV_URL,
};

/// A source of USD pricing for `(provider, model)` calls. Boxed as a trait
/// object on `AppState` so HTTP fixtures can inject a fixed catalog instead
/// of reaching the network, the same seam `HostMetricsProvider` uses.
#[async_trait]
pub trait ModelPricingProvider: Send + Sync + 'static {
    #[allow(clippy::too_many_arguments)]
    async fn cost_usd(
        &self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
        reasoning_tokens: u64,
    ) -> Option<f64>;

    /// Net USD saved (or lost) by this call's cache activity versus billing
    /// it all at the plain input rate. `tokens_in` is needed to resolve the
    /// same tier `cost_usd` would use for the same call.
    async fn cache_savings_usd(
        &self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    ) -> Option<f64>;

    /// The full catalog for a reference-price screen, refreshing first if
    /// the cache is stale (the same lazy policy `cost_usd` uses).
    async fn snapshot(&self) -> ModelPricingCatalogSnapshot;

    /// Force a fetch regardless of cache age, then return the catalog —
    /// for a user-initiated "refresh" action.
    async fn refresh(&self) -> ModelPricingCatalogSnapshot;
}

/// Prices one `model_call`, treating "consumed no tokens at all" as its own
/// case rather than routing it through the catalog.
///
/// A request that errored before the model returned anything reports
/// `tokens_in = tokens_out = 0` — nothing was consumed, so the cost is $0
/// for certain, whatever the catalog does or does not know about the model.
/// Without this, such a call sits in "unpriced" forever: a rate lookup for
/// its provider/model can fail for reasons that have nothing to do with the
/// call itself (a preview model models.dev has not listed yet, for
/// instance), and there would be no way to tell "definitely free" apart
/// from "rate unknown" by looking at the stored `NULL` alone.
#[allow(clippy::too_many_arguments)]
pub async fn price_model_call(
    pricing: &dyn ModelPricingProvider,
    provider: &str,
    model: &str,
    tokens_in: u64,
    tokens_out: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
) -> Option<f64> {
    if tokens_in == 0 && tokens_out == 0 {
        return Some(0.0);
    }
    pricing
        .cost_usd(
            provider,
            model,
            tokens_in,
            tokens_out,
            cache_read_tokens,
            cache_write_tokens,
            reasoning_tokens,
        )
        .await
}

/// Net USD saved by one `model_call`'s cache activity, treating "no cache
/// read or write at all" as its own zero-savings case rather than routing
/// it through the catalog — mirrors `price_model_call`'s zero-token
/// shortcut and for the same reason: a call with no cache activity has
/// definitely saved (and cost) nothing extra, whatever the catalog does or
/// does not know about the model.
pub async fn cache_savings_for_call(
    pricing: &dyn ModelPricingProvider,
    provider: &str,
    model: &str,
    tokens_in: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
) -> Option<f64> {
    if cache_read_tokens == 0 && cache_write_tokens == 0 {
        return Some(0.0);
    }
    pricing
        .cache_savings_usd(provider, model, tokens_in, cache_read_tokens, cache_write_tokens)
        .await
}

type CatalogMap = HashMap<(String, String), ModelPricing>;

struct CatalogState {
    models: CatalogMap,
    fetched_at: Option<Instant>,
    /// Wall-clock twin of `fetched_at` — an `Instant` cannot be shown to a
    /// user, so this is what a catalog screen displays as "last updated".
    fetched_at_utc: Option<DateTime<Utc>>,
}

fn catalog_snapshot(models: &CatalogMap, fetched_at_utc: Option<DateTime<Utc>>) -> ModelPricingCatalogSnapshot {
    let mut entries: Vec<ModelPricingCatalogEntry> = models
        .iter()
        .map(|((provider, model), pricing)| ModelPricingCatalogEntry {
            provider: provider.clone(),
            model: model.clone(),
            pricing: pricing.clone(),
        })
        .collect();
    entries.sort_by(|a, b| (&a.provider, &a.model).cmp(&(&b.provider, &b.model)));
    ModelPricingCatalogSnapshot {
        entries,
        fetched_at: fetched_at_utc,
    }
}

/// Fetches and caches the public models.dev catalog, keeping only what
/// pricing needs (`cost` per model — no limits, thinking, or feature data).
pub struct ModelsDevPricingCatalog {
    cache_path: PathBuf,
    client: reqwest::Client,
    state: Mutex<CatalogState>,
}

impl ModelsDevPricingCatalog {
    pub fn new(cache_path: impl Into<PathBuf>) -> Self {
        let cache_path = cache_path.into();
        let mut state = CatalogState {
            models: HashMap::new(),
            fetched_at: None,
            fetched_at_utc: None,
        };
        if let Some((models, modified)) = read_cache_file(&cache_path) {
            state.models = models;
            // A cache file still inside the TTL means a fresh process does
            // not have to hit the network before it can price anything.
            let fresh = modified
                .elapsed()
                .map(|age| age < Duration::from_secs(MODELS_DEV_CACHE_TTL_SECONDS))
                .unwrap_or(false);
            if fresh {
                state.fetched_at = Some(Instant::now());
                state.fetched_at_utc = Some(DateTime::<Utc>::from(modified));
            }
        }
        Self {
            cache_path,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(MODELS_DEV_FETCH_TIMEOUT_SECONDS))
                .build()
                .unwrap_or_default(),
            state: Mutex::new(state),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, CatalogState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn ensure_fresh(&self) {
        let stale = match self.lock_state().fetched_at {
            Some(at) => at.elapsed() >= Duration::from_secs(MODELS_DEV_CACHE_TTL_SECONDS),
            None => true,
        };
        if stale {
            self.force_refresh().await;
        }
    }

    /// Fetches unconditionally, ignoring cache age — the "refresh" action,
    /// and also what a stale `ensure_fresh` falls through to.
    async fn force_refresh(&self) {
        match self.fetch_body().await {
            Ok(body) => {
                let models = parse_catalog(&body);
                write_cache_file(&self.cache_path, &body);
                let now = Utc::now();
                let mut state = self.lock_state();
                state.models = models;
                state.fetched_at = Some(Instant::now());
                state.fetched_at_utc = Some(now);
            }
            Err(error) => {
                tracing::warn!(%error, "failed to refresh models.dev pricing catalog");
                // Keep serving whatever is cached (possibly nothing); back
                // off so a persistent outage does not retry on every event.
                self.lock_state().fetched_at = Some(Instant::now());
            }
        }
    }

    async fn fetch_body(&self) -> Result<String, reqwest::Error> {
        let response = self.client.get(MODELS_DEV_URL).send().await?;
        let response = response.error_for_status()?;
        response.text().await
    }
}

#[async_trait]
impl ModelPricingProvider for ModelsDevPricingCatalog {
    async fn cost_usd(
        &self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
        reasoning_tokens: u64,
    ) -> Option<f64> {
        self.ensure_fresh().await;
        let key = (provider.to_ascii_lowercase(), model.to_ascii_lowercase());
        let pricing = self.lock_state().models.get(&key).cloned()?;
        let rates = resolve_rates(&pricing, tokens_in);
        estimate_cost_usd(
            &rates,
            tokens_in,
            tokens_out,
            cache_read_tokens,
            cache_write_tokens,
            reasoning_tokens,
        )
    }

    async fn cache_savings_usd(
        &self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    ) -> Option<f64> {
        self.ensure_fresh().await;
        let key = (provider.to_ascii_lowercase(), model.to_ascii_lowercase());
        let pricing = self.lock_state().models.get(&key).cloned()?;
        let rates = resolve_rates(&pricing, tokens_in);
        conductor_domain::cache_savings_usd(&rates, cache_read_tokens, cache_write_tokens)
    }

    async fn snapshot(&self) -> ModelPricingCatalogSnapshot {
        self.ensure_fresh().await;
        let state = self.lock_state();
        catalog_snapshot(&state.models, state.fetched_at_utc)
    }

    async fn refresh(&self) -> ModelPricingCatalogSnapshot {
        self.force_refresh().await;
        let state = self.lock_state();
        catalog_snapshot(&state.models, state.fetched_at_utc)
    }
}

/// A fixed catalog for tests — never touches the network or disk.
#[derive(Default, Clone)]
pub struct StaticPricingCatalog {
    models: CatalogMap,
}

impl StaticPricingCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_model(
        mut self,
        provider: impl Into<String>,
        model: impl Into<String>,
        pricing: ModelPricing,
    ) -> Self {
        self.models.insert(
            (
                provider.into().to_ascii_lowercase(),
                model.into().to_ascii_lowercase(),
            ),
            pricing,
        );
        self
    }
}

#[async_trait]
impl ModelPricingProvider for StaticPricingCatalog {
    async fn cost_usd(
        &self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
        reasoning_tokens: u64,
    ) -> Option<f64> {
        let key = (provider.to_ascii_lowercase(), model.to_ascii_lowercase());
        let pricing = self.models.get(&key)?;
        let rates = resolve_rates(pricing, tokens_in);
        estimate_cost_usd(
            &rates,
            tokens_in,
            tokens_out,
            cache_read_tokens,
            cache_write_tokens,
            reasoning_tokens,
        )
    }

    async fn cache_savings_usd(
        &self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    ) -> Option<f64> {
        let key = (provider.to_ascii_lowercase(), model.to_ascii_lowercase());
        let pricing = self.models.get(&key)?;
        let rates = resolve_rates(pricing, tokens_in);
        conductor_domain::cache_savings_usd(&rates, cache_read_tokens, cache_write_tokens)
    }

    async fn snapshot(&self) -> ModelPricingCatalogSnapshot {
        catalog_snapshot(&self.models, None)
    }

    async fn refresh(&self) -> ModelPricingCatalogSnapshot {
        catalog_snapshot(&self.models, None)
    }
}

fn read_cache_file(path: &Path) -> Option<(CatalogMap, SystemTime)> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let body = std::fs::read_to_string(path).ok()?;
    Some((parse_catalog(&body), modified))
}

fn write_cache_file(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            tracing::warn!(%error, path = %parent.display(), "failed to create models.dev cache directory");
            return;
        }
    }
    if let Err(error) = std::fs::write(path, body) {
        tracing::warn!(%error, path = %path.display(), "failed to write models.dev pricing cache");
    }
}

/// evoflux's own provider id, mapped to the id models.dev uses for the same
/// vendor, for every built-in provider where the two disagree. Ported
/// verbatim from the `models_dev_id` values in
/// `app/agent/providers/registry.py` (evoflux repo) — that file is the
/// authority on this mapping; a new mismatched provider added there needs
/// its pair added here too.
///
/// Kept as a static list rather than the dynamic sibling/plugin resolution
/// evoflux also does: those cover third-party plugin providers and
/// same-vendor regional/plan variants, neither of which conductor's own
/// catalog needs.
const EVOFLUX_PROVIDER_ALIASES: &[(&str, &str)] = &[
    ("googlegenai", "google"),
    ("kimi", "kimi-for-coding"),
    ("moonshot", "moonshotai"),
    ("qwencloud", "alibaba"),
    ("together", "togetherai"),
    ("fireworks", "fireworks-ai"),
    ("bedrock", "amazon-bedrock"),
    ("vertexai", "google-vertex"),
    ("foundry", "azure-cognitive-services"),
    ("copilot", "github-copilot"),
    ("codex", "openai"),
];

/// `data[provider_key]["models"][model_key]["cost"]` for every model that
/// states a cost, keyed by lowercased `(provider, model)` id — the same
/// pair evoflux reports on a `model_call` event (`provider`/`model`, split
/// from its own qualified `provider:model` string).
///
/// Every model priced under a models.dev provider id that
/// `EVOFLUX_PROVIDER_ALIASES` maps from an evoflux provider id is also
/// inserted under that evoflux id, so a lookup by the id evoflux actually
/// reports (`googlegenai`, not `google`) still finds it.
fn parse_catalog(body: &str) -> CatalogMap {
    let mut result = HashMap::new();
    let Ok(Value::Object(providers)) = serde_json::from_str::<Value>(body) else {
        return result;
    };
    for (provider_key, provider) in providers {
        let Some(provider_obj) = provider.as_object() else {
            continue;
        };
        let provider_id = provider_obj
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(&provider_key)
            .to_ascii_lowercase();
        let Some(models) = provider_obj.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (model_key, model) in models {
            let Some(model_obj) = model.as_object() else {
                continue;
            };
            let model_id = model_obj
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(model_key.as_str())
                .to_ascii_lowercase();
            if let Some(pricing) = cost_from_model(model_obj) {
                result.insert((provider_id.clone(), model_id), pricing);
            }
        }
    }
    apply_provider_aliases(&mut result);
    result
}

fn apply_provider_aliases(catalog: &mut CatalogMap) {
    for (evoflux_id, models_dev_id) in EVOFLUX_PROVIDER_ALIASES {
        let matches: Vec<(String, ModelPricing)> = catalog
            .iter()
            .filter(|((provider, _), _)| provider == models_dev_id)
            .map(|((_, model), pricing)| (model.clone(), pricing.clone()))
            .collect();
        for (model, pricing) in matches {
            catalog
                .entry(((*evoflux_id).to_string(), model))
                .or_insert(pricing);
        }
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64)
}

fn cost_rates_from(map: &Map<String, Value>) -> ModelCostRates {
    ModelCostRates {
        input: number(map.get("input")),
        output: number(map.get("output")),
        cache_read: number(map.get("cache_read")),
        cache_write: number(map.get("cache_write")),
        reasoning: number(map.get("reasoning")),
    }
}

fn has_any_rate(rates: &ModelCostRates) -> bool {
    rates.input.is_some()
        || rates.output.is_some()
        || rates.cache_read.is_some()
        || rates.cache_write.is_some()
        || rates.reasoning.is_some()
}

/// One entry of `cost.tiers` — a rate that applies past a threshold. Mirrors
/// `_cost_tier` in `model_registry.py`.
fn cost_tier_from(tier: &Map<String, Value>) -> Option<CostTier> {
    let rates = cost_rates_from(tier);
    if !has_any_rate(&rates) {
        return None;
    }
    let above_tokens = tier
        .get("tier")
        .and_then(Value::as_object)
        .and_then(|size| size.get("size"))
        .and_then(Value::as_u64)?;
    Some(CostTier { above_tokens, rates })
}

/// Mirrors `_cost_from_model`: normalizes both spellings models.dev uses for
/// a long-context surcharge — the newer `tiers` list and the older
/// `context_over_200k` object — into the same tier shape, preferring an
/// explicit 200K-token tier over the legacy field when a row states both.
fn cost_from_model(model: &Map<String, Value>) -> Option<ModelPricing> {
    let cost = model.get("cost").and_then(Value::as_object)?;
    let base = cost_rates_from(cost);

    let mut tiers: Vec<CostTier> = Vec::new();
    if let Some(raw_tiers) = cost.get("tiers").and_then(Value::as_array) {
        for item in raw_tiers {
            if let Some(map) = item.as_object() {
                if let Some(tier) = cost_tier_from(map) {
                    tiers.push(tier);
                }
            }
        }
    }
    if let Some(over_200k) = cost.get("context_over_200k").and_then(Value::as_object) {
        let rates = cost_rates_from(over_200k);
        if has_any_rate(&rates) && !tiers.iter().any(|tier| tier.above_tokens == 200_000) {
            tiers.push(CostTier {
                above_tokens: 200_000,
                rates,
            });
        }
    }

    if !has_any_rate(&base) && tiers.is_empty() {
        return None;
    }
    Some(ModelPricing { base, tiers })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_cost() {
        let body = r#"{
            "anthropic": {
                "models": {
                    "claude-sonnet-5": {
                        "cost": {"input": 3.0, "output": 15.0, "cache_read": 0.3}
                    }
                }
            }
        }"#;
        let catalog = parse_catalog(body);
        let pricing = catalog
            .get(&("anthropic".to_string(), "claude-sonnet-5".to_string()))
            .expect("model present");
        assert_eq!(pricing.base.input, Some(3.0));
        assert_eq!(pricing.base.output, Some(15.0));
        assert_eq!(pricing.base.cache_read, Some(0.3));
        assert!(pricing.tiers.is_empty());
    }

    #[test]
    fn parses_tiers_list() {
        let body = r#"{
            "openai": {
                "models": {
                    "gpt-5": {
                        "cost": {
                            "input": 3.0,
                            "output": 15.0,
                            "tiers": [{"input": 6.0, "output": 22.5, "tier": {"size": 200000}}]
                        }
                    }
                }
            }
        }"#;
        let catalog = parse_catalog(body);
        let pricing = catalog
            .get(&("openai".to_string(), "gpt-5".to_string()))
            .expect("model present");
        assert_eq!(pricing.tiers.len(), 1);
        assert_eq!(pricing.tiers[0].above_tokens, 200_000);
        assert_eq!(pricing.tiers[0].rates.input, Some(6.0));
    }

    #[test]
    fn googlegenai_resolves_to_the_models_dev_google_row() {
        let body = r#"{
            "google": {
                "models": {
                    "gemini-3.8-flash": {
                        "cost": {"input": 0.75, "output": 3.75, "cache_read": 0.075}
                    }
                }
            }
        }"#;
        let catalog = parse_catalog(body);
        let pricing = catalog
            .get(&("googlegenai".to_string(), "gemini-3.8-flash".to_string()))
            .expect("aliased provider resolves to the google row");
        assert_eq!(pricing.base.input, Some(0.75));
        // The models.dev row itself is still reachable directly.
        assert!(catalog.contains_key(&("google".to_string(), "gemini-3.8-flash".to_string())));
    }

    #[test]
    fn a_provider_row_with_no_alias_is_unaffected() {
        let body = r#"{
            "anthropic": {
                "models": {
                    "claude-sonnet-5": {"cost": {"input": 2.0, "output": 10.0}}
                }
            }
        }"#;
        let catalog = parse_catalog(body);
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn legacy_context_over_200k_becomes_a_tier() {
        let body = r#"{
            "vendor": {
                "models": {
                    "model-a": {
                        "cost": {
                            "input": 3.0,
                            "output": 15.0,
                            "context_over_200k": {"input": 6.0, "output": 22.5}
                        }
                    }
                }
            }
        }"#;
        let catalog = parse_catalog(body);
        let pricing = catalog
            .get(&("vendor".to_string(), "model-a".to_string()))
            .unwrap();
        assert_eq!(pricing.tiers.len(), 1);
        assert_eq!(pricing.tiers[0].above_tokens, 200_000);
    }

    #[test]
    fn explicit_200k_tier_wins_over_legacy_field() {
        let body = r#"{
            "vendor": {
                "models": {
                    "model-a": {
                        "cost": {
                            "input": 3.0,
                            "tiers": [{"input": 5.0, "tier": {"size": 200000}}],
                            "context_over_200k": {"input": 9.0}
                        }
                    }
                }
            }
        }"#;
        let catalog = parse_catalog(body);
        let pricing = catalog
            .get(&("vendor".to_string(), "model-a".to_string()))
            .unwrap();
        assert_eq!(pricing.tiers.len(), 1);
        assert_eq!(pricing.tiers[0].rates.input, Some(5.0));
    }

    #[test]
    fn model_without_cost_is_absent() {
        let body = r#"{"vendor": {"models": {"model-a": {"name": "Model A"}}}}"#;
        let catalog = parse_catalog(body);
        assert!(catalog.is_empty());
    }

    #[tokio::test]
    async fn static_catalog_prices_a_known_model() {
        let catalog = StaticPricingCatalog::new().with_model(
            "anthropic",
            "claude-sonnet-5",
            ModelPricing {
                base: ModelCostRates {
                    input: Some(3.0),
                    output: Some(15.0),
                    ..Default::default()
                },
                tiers: Vec::new(),
            },
        );
        let cost = catalog
            .cost_usd("Anthropic", "Claude-Sonnet-5", 1_000_000, 1_000_000, 0, 0, 0)
            .await;
        assert_eq!(cost, Some(18.0));
    }

    #[tokio::test]
    async fn static_catalog_returns_none_for_unknown_model() {
        let catalog = StaticPricingCatalog::new();
        let cost = catalog.cost_usd("anthropic", "unknown-model", 1000, 1000, 0, 0, 0).await;
        assert_eq!(cost, None);
    }

    #[tokio::test]
    async fn zero_token_call_prices_at_zero_even_for_an_unknown_model() {
        let catalog = StaticPricingCatalog::new();
        let cost =
            price_model_call(&catalog, "anthropic", "unknown-preview-model", 0, 0, 0, 0, 0).await;
        assert_eq!(cost, Some(0.0));
    }

    #[tokio::test]
    async fn nonzero_token_call_still_defers_to_the_catalog() {
        let catalog = StaticPricingCatalog::new();
        let cost = price_model_call(&catalog, "anthropic", "unknown-model", 100, 0, 0, 0, 0).await;
        assert_eq!(cost, None);
    }

    #[tokio::test]
    async fn cache_savings_uses_the_catalog_rates() {
        let catalog = StaticPricingCatalog::new().with_model(
            "anthropic",
            "claude-sonnet-5",
            ModelPricing {
                base: ModelCostRates {
                    input: Some(3.0),
                    output: Some(15.0),
                    cache_read: Some(0.3),
                    ..Default::default()
                },
                tiers: Vec::new(),
            },
        );
        let savings =
            cache_savings_for_call(&catalog, "anthropic", "claude-sonnet-5", 1_000_000, 1_000_000, 0)
                .await
                .unwrap();
        // 1,000,000 cache-read tokens at $0.3/M instead of $3/M input.
        assert!((savings - 2.7).abs() < 1e-9);
    }

    #[tokio::test]
    async fn cache_savings_unknown_model_with_activity_is_none() {
        let catalog = StaticPricingCatalog::new();
        let savings =
            cache_savings_for_call(&catalog, "anthropic", "unknown-model", 1000, 100, 0).await;
        assert_eq!(savings, None);
    }

    #[tokio::test]
    async fn cache_savings_no_activity_is_zero_even_for_an_unknown_model() {
        let catalog = StaticPricingCatalog::new();
        let savings =
            cache_savings_for_call(&catalog, "anthropic", "unknown-model", 1000, 0, 0).await;
        assert_eq!(savings, Some(0.0));
    }
}
