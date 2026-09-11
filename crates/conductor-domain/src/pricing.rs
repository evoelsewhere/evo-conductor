//! Server-side model pricing.
//!
//! Conductor prices telemetry itself instead of trusting the client's
//! `estimated_cost_usd_micros`, so the fleet shares one rate table and history
//! can be repriced. The arithmetic here mirrors EvoFlux's `estimate_cost`
//! (`app/agent/usage.py`) deliberately: if the two disagree, a member's local
//! receipt stops reconciling with the project's reported spend and neither
//! number is trustworthy.

use serde::{Deserialize, Serialize};

/// One million, the denominator of a models.dev rate.
const TOKENS_PER_RATE_UNIT: i128 = 1_000_000;

/// Rates for one model, as micro-USD per million tokens.
///
/// models.dev quotes USD per million tokens (`0.4`, `2.65`); scaling by 1e6
/// keeps money in integers, and `cost_micros = tokens * usd_per_million`
/// numerically, so no precision is lost to the unit choice.
///
/// A `None` rate means models.dev published no price for that component.
/// `tool_use` is absent because no provider prices it: EvoFlux counts
/// `tool_use_tokens` but never bills them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRates {
    pub input: Option<i64>,
    pub output: Option<i64>,
    pub cache_read: Option<i64>,
    pub cache_write: Option<i64>,
    pub reasoning: Option<i64>,
}

impl ModelRates {
    pub fn is_empty(&self) -> bool {
        self.input.is_none()
            && self.output.is_none()
            && self.cache_read.is_none()
            && self.cache_write.is_none()
            && self.reasoning.is_none()
    }

    /// Lay `other` over `self`, component by component: a rate `other`
    /// publishes replaces this one, a rate it omits leaves this one standing.
    ///
    /// This is how both a long-context tier and a service tier are applied,
    /// and the per-component part is load-bearing. A tier that quotes only
    /// `input` and `output` is not saying cache reads are free -- it is
    /// saying nothing about them, so the headline cache rate still applies.
    /// Replacing the whole rate set instead would silently unprice every
    /// component the tier happens not to mention.
    #[must_use]
    pub fn overlay(self, other: &ModelRates) -> ModelRates {
        ModelRates {
            input: other.input.or(self.input),
            output: other.output.or(self.output),
            cache_read: other.cache_read.or(self.cache_read),
            cache_write: other.cache_write.or(self.cache_write),
            reasoning: other.reasoning.or(self.reasoning),
        }
    }
}

/// A long-context tier: once a request's prompt exceeds `above_tokens`, the
/// provider bills **the whole request** at these rates -- not just the tokens
/// past the threshold.
///
/// That distinction is the whole point. Treating the surcharge as marginal
/// (`threshold x base + overflow x above`) understates a 300K-token Sonnet
/// turn by roughly a third, and no provider prices it that way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateTier {
    /// Prompt-token threshold. Applies when prompt tokens are strictly above
    /// it, matching the client.
    pub above_tokens: i64,
    /// The rates this band publishes, overlaid on the headline rates.
    pub rates: ModelRates,
}

/// An alternate service tier -- a fast or priority lane billed at its own
/// rates. Named exactly as the client reports it in `service_tier`, after
/// [`normalize_model_key`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceTierRates {
    pub tier: String,
    pub rates: ModelRates,
}

/// Everything published about one model's price: the headline rates plus the
/// bands and lanes that can move a request off them.
///
/// `tiers` is sorted by `above_tokens` ascending and `service_tiers` by name,
/// so two catalogs that publish the same prices in a different order compare
/// equal and do not manufacture a spurious rate change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPricing {
    pub base: ModelRates,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiers: Vec<RateTier>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_tiers: Vec<ServiceTierRates>,
}

impl ModelPricing {
    pub fn flat(base: ModelRates) -> Self {
        Self {
            base,
            ..Self::default()
        }
    }

    /// True when upstream publishes no usable rate anywhere -- headline,
    /// band, or lane. A model whose only rate lives in a tier is still
    /// priced, so this cannot just test `base`.
    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
            && self.tiers.iter().all(|tier| tier.rates.is_empty())
            && self
                .service_tiers
                .iter()
                .all(|service_tier| service_tier.rates.is_empty())
    }

    /// Put `tiers` and `service_tiers` in their canonical order. Call this
    /// after building a value from an upstream document.
    pub fn sorted(mut self) -> Self {
        self.tiers.sort_by_key(|tier| tier.above_tokens);
        self.service_tiers
            .sort_by(|left, right| left.tier.cmp(&right.tier));
        self
    }
}

/// The rates that actually apply to one request.
///
/// Order mirrors EvoFlux's `_tiered_rates`: the highest applicable
/// long-context band first, then the service tier, which wins because it is a
/// different product rather than a volume band. A request over 400K is also
/// over 200K, so the *highest* threshold it clears is the one billed.
pub fn resolve_rates(
    pricing: &ModelPricing,
    prompt_tokens: i64,
    service_tier: Option<&str>,
) -> ModelRates {
    let mut rates = pricing.base;

    if let Some(tier) = pricing
        .tiers
        .iter()
        .filter(|tier| prompt_tokens > tier.above_tokens)
        .max_by_key(|tier| tier.above_tokens)
    {
        rates = rates.overlay(&tier.rates);
    }

    if let Some(requested) = service_tier {
        let requested = normalize_model_key(requested);
        if let Some(lane) = pricing
            .service_tiers
            .iter()
            .find(|lane| lane.tier == requested)
        {
            rates = rates.overlay(&lane.rates);
        }
    }

    rates
}

/// Price one model call against a model's full published pricing, resolving
/// the long-context band and service tier first.
///
/// The band is chosen from `usage.tokens_in`, which is the request's whole
/// prompt (cache reads and writes included) -- the same count the provider
/// measures context length by, and the same one the client uses.
///
/// This is the entry point for pricing a **single call**. Do not reach for it
/// with a summed usage total: once bands exist, pricing an aggregate is no
/// longer equivalent to pricing each call and summing, because the sum of
/// many small prompts can clear a threshold that none of them did.
pub fn price_model_call_tiered(
    pricing: &ModelPricing,
    usage: TokenUsage,
    service_tier: Option<&str>,
) -> PricedCost {
    if pricing.is_empty() {
        return PricedCost::Unpriced {
            reason: UnpricedReason::NoRates,
        };
    }
    price_model_call(
        usage,
        &resolve_rates(pricing, usage.tokens_in, service_tier),
    )
}

/// Token counts exactly as EvoFlux reports them: `tokens_in` already includes
/// cache reads and cache writes, `tokens_out` already includes reasoning
/// tokens. Conductor's own aggregates rely on the same rule, which is why
/// summing every counter would double count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
}

/// Why an event carries no server-computed cost. Kept distinct from "cost is
/// zero" so unpriced volume stays visible instead of silently reading as free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnpricedReason {
    /// No price catalog has been synced yet.
    NoCatalog,
    /// The catalog has no row for this provider/model.
    ModelUnknown,
    /// The model is known but models.dev publishes no usable rate.
    NoRates,
    /// Rates exist, but not for any component this event actually used.
    NoMatchingComponent,
}

impl UnpricedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoCatalog => "no_catalog",
            Self::ModelUnknown => "model_unknown",
            Self::NoRates => "no_rates",
            Self::NoMatchingComponent => "no_matching_component",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "no_catalog" => Some(Self::NoCatalog),
            "model_unknown" => Some(Self::ModelUnknown),
            "no_rates" => Some(Self::NoRates),
            "no_matching_component" => Some(Self::NoMatchingComponent),
            _ => None,
        }
    }
}

/// Which rate a stored cost was computed from.
///
/// models.dev publishes only current prices, so Conductor cannot know what a
/// model cost last month. Events older than its first sync can therefore only
/// be *estimated*, and that has to be visible in the data: a figure derived
/// from the earliest rate ever observed is not the same claim as one derived
/// from the rate actually in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingBasis {
    /// The rate whose effective period covers the event.
    InForce,
    /// The earliest rate Conductor ever recorded, applied to an event that
    /// predates it. An estimate, not a price.
    EarliestKnown,
}

impl PricingBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InForce => "in_force",
            Self::EarliestKnown => "earliest_known",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "in_force" => Some(Self::InForce),
            "earliest_known" => Some(Self::EarliestKnown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PricedCost {
    Priced { cost_micros: i64 },
    Unpriced { reason: UnpricedReason },
}

impl PricedCost {
    pub fn cost_micros(self) -> Option<i64> {
        match self {
            Self::Priced { cost_micros } => Some(cost_micros),
            Self::Unpriced { .. } => None,
        }
    }

    pub fn unpriced_reason(self) -> Option<UnpricedReason> {
        match self {
            Self::Priced { .. } => None,
            Self::Unpriced { reason } => Some(reason),
        }
    }
}

/// Price one model call.
///
/// Component order and the conditional subtraction below are load-bearing and
/// copied from EvoFlux: cache and reasoning tokens are only carved out of the
/// input/output totals **when a rate for them exists**. Where models.dev
/// publishes no cache_read rate, those tokens stay billed as ordinary input —
/// changing that here would make Conductor disagree with every client.
pub fn price_model_call(usage: TokenUsage, rates: &ModelRates) -> PricedCost {
    if rates.is_empty() {
        return PricedCost::Unpriced {
            reason: UnpricedReason::NoRates,
        };
    }
    let scaled = scaled_cost_components(usage, rates);
    if !scaled.priced_component {
        return PricedCost::Unpriced {
            reason: UnpricedReason::NoMatchingComponent,
        };
    }
    PricedCost::Priced {
        cost_micros: round_scaled(scaled.total()),
    }
}

/// Each carved-out component, in scaled micro-USD times 1e6 -- not yet
/// divided down to micro-USD. A caller aggregating many events sums these
/// before dividing, the same way `price_model_call` rounds its total once
/// rather than truncating every event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ScaledCostComponents {
    input: i128,
    output: i128,
    cache_read: i128,
    cache_write: i128,
    reasoning: i128,
    /// At least one component had a rate to price it against.
    priced_component: bool,
}

impl ScaledCostComponents {
    fn total(&self) -> i128 {
        self.input + self.output + self.cache_read + self.cache_write + self.reasoning
    }
}

/// The carve-out logic shared by `price_model_call` and `price_components`.
/// Component order and the conditional subtraction are load-bearing and
/// copied from EvoFlux: cache and reasoning tokens are only carved out of the
/// input/output totals **when a rate for them exists**. Where models.dev
/// publishes no cache_read rate, those tokens stay billed as ordinary input --
/// changing that here would make Conductor disagree with every client.
fn scaled_cost_components(usage: TokenUsage, rates: &ModelRates) -> ScaledCostComponents {
    let mut billable_input = usage.tokens_in.max(0) as i128;
    let mut billable_output = usage.tokens_out.max(0) as i128;
    let cache_read = usage.cache_read.max(0) as i128;
    let cache_write = usage.cache_write.max(0) as i128;
    let reasoning = usage.reasoning.max(0) as i128;

    let mut components = ScaledCostComponents::default();

    if let (Some(rate), true) = (rates.cache_read, cache_read > 0) {
        components.cache_read = cache_read * i128::from(rate);
        billable_input = (billable_input - cache_read).max(0);
        components.priced_component = true;
    }
    if let (Some(rate), true) = (rates.cache_write, cache_write > 0) {
        components.cache_write = cache_write * i128::from(rate);
        billable_input = (billable_input - cache_write).max(0);
        components.priced_component = true;
    }
    if let (Some(rate), true) = (rates.input, billable_input > 0) {
        components.input = billable_input * i128::from(rate);
        components.priced_component = true;
    }
    if let (Some(rate), true) = (rates.reasoning, reasoning > 0) {
        components.reasoning = reasoning * i128::from(rate);
        billable_output = (billable_output - reasoning).max(0);
        components.priced_component = true;
    }
    if let (Some(rate), true) = (rates.output, billable_output > 0) {
        components.output = billable_output * i128::from(rate);
        components.priced_component = true;
    }

    components
}

/// Round half up: truncating would under-report systematically once summed
/// over millions of events.
fn round_scaled(scaled: i128) -> i64 {
    let cost = (scaled + TOKENS_PER_RATE_UNIT / 2) / TOKENS_PER_RATE_UNIT;
    cost.clamp(0, i128::from(i64::MAX)) as i64
}

/// Where a priced cost actually came from: input, output, or a carved-out
/// cache/reasoning component. Each field is independently rounded, so the sum
/// of all five can differ from `price_model_call`'s single-shot total by a
/// few micro-USD -- invisible at dollar precision. A caller that must
/// reconcile exactly against a stored authoritative total should scale these
/// proportionally rather than trust the sum bit for bit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CostComponents {
    pub input_usd_micros: i64,
    pub output_usd_micros: i64,
    pub cache_read_usd_micros: i64,
    pub cache_write_usd_micros: i64,
    pub reasoning_usd_micros: i64,
}

impl CostComponents {
    pub fn total_usd_micros(&self) -> i64 {
        self.input_usd_micros
            + self.output_usd_micros
            + self.cache_read_usd_micros
            + self.cache_write_usd_micros
            + self.reasoning_usd_micros
    }
}

/// Price one usage total into its component contributions rather than a
/// single number, against **one already-resolved rate set**.
///
/// `usage` may be a single event or a sum of many events for the same model:
/// the carve-out rule depends only on which rates exist, so for a fixed rate
/// set, summing tokens first and pricing once equals summing each event's
/// components.
///
/// That equivalence holds for the rates handed in, not for the model. A model
/// with a long-context band has no single rate set -- which one applies is a
/// property of each individual request's prompt size, and a sum of many small
/// prompts can clear a threshold that no single call did. So this must never
/// be called with a summed usage and a band-resolved rate set. Its one caller
/// that does aggregate, the model cost report, uses the result solely as a
/// **split ratio** over a total storage already summed per event; see
/// `split_components` there.
pub fn price_components(usage: TokenUsage, rates: &ModelRates) -> Option<CostComponents> {
    if rates.is_empty() {
        return None;
    }
    let scaled = scaled_cost_components(usage, rates);
    if !scaled.priced_component {
        return None;
    }
    Some(CostComponents {
        input_usd_micros: round_scaled(scaled.input),
        output_usd_micros: round_scaled(scaled.output),
        cache_read_usd_micros: round_scaled(scaled.cache_read),
        cache_write_usd_micros: round_scaled(scaled.cache_write),
        reasoning_usd_micros: round_scaled(scaled.reasoning),
    })
}

/// Key a model is stored and looked up under.
///
/// EvoFlux reports `provider`/`model` as free text straight from whichever SDK
/// produced the call, so casing and padding vary between providers and
/// versions. Normalising on both write and read keeps `OpenAI`/`openai` from
/// becoming two unrelated rate rows, which would show up as phantom unpriced
/// volume rather than as an error.
pub fn normalize_model_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// Convert a models.dev rate (USD per million tokens) to the integer
/// micro-USD-per-million this module prices with. Rejects negatives, NaN and
/// values large enough to make later arithmetic meaningless.
pub fn rate_from_usd_per_million(value: f64) -> Option<i64> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let scaled = (value * 1_000_000.0).round();
    if scaled > i64::MAX as f64 {
        return None;
    }
    Some(scaled as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Anthropic-shaped rates: cache reads cheap, cache writes dearer than
    /// input. Values are USD per million tokens, scaled.
    fn sonnet_rates() -> ModelRates {
        ModelRates {
            input: rate_from_usd_per_million(3.0),
            output: rate_from_usd_per_million(15.0),
            cache_read: rate_from_usd_per_million(0.3),
            cache_write: rate_from_usd_per_million(3.75),
            reasoning: None,
        }
    }

    #[test]
    fn scales_a_models_dev_rate_into_micro_usd_per_million() {
        assert_eq!(rate_from_usd_per_million(0.4), Some(400_000));
        assert_eq!(rate_from_usd_per_million(2.65), Some(2_650_000));
        assert_eq!(rate_from_usd_per_million(0.0), Some(0));
        assert_eq!(rate_from_usd_per_million(-1.0), None);
        assert_eq!(rate_from_usd_per_million(f64::NAN), None);
        assert_eq!(rate_from_usd_per_million(f64::INFINITY), None);
    }

    /// One million input tokens at $3/M is exactly $3, i.e. 3,000,000 micros.
    #[test]
    fn prices_a_round_million_input_tokens_exactly() {
        let cost = price_model_call(
            TokenUsage {
                tokens_in: 1_000_000,
                ..TokenUsage::default()
            },
            &sonnet_rates(),
        );
        assert_eq!(cost.cost_micros(), Some(3_000_000));
    }

    /// The disjointness rule: cache reads and writes come out of `tokens_in`,
    /// reasoning out of `tokens_out`, so nothing is billed twice.
    #[test]
    fn carves_cache_and_reasoning_out_of_the_totals() {
        let rates = ModelRates {
            reasoning: rate_from_usd_per_million(30.0),
            ..sonnet_rates()
        };
        let cost = price_model_call(
            TokenUsage {
                tokens_in: 1_000_000,
                tokens_out: 1_000_000,
                cache_read: 400_000,
                cache_write: 100_000,
                reasoning: 250_000,
            },
            &rates,
        );

        // input:   500,000 x $3     = $1.50
        // cache_r: 400,000 x $0.30  = $0.12
        // cache_w: 100,000 x $3.75  = $0.375
        // reason:  250,000 x $30    = $7.50
        // output:  750,000 x $15    = $11.25
        let expected = 1_500_000 + 120_000 + 375_000 + 7_500_000 + 11_250_000;
        assert_eq!(cost.cost_micros(), Some(expected));
    }

    /// EvoFlux only carves out a component when it has a rate. With no
    /// cache_read rate, cached tokens must stay billed as ordinary input —
    /// diverging here would silently undercharge every such model.
    #[test]
    fn without_a_cache_rate_cached_tokens_are_billed_as_input() {
        let rates = ModelRates {
            cache_read: None,
            cache_write: None,
            ..sonnet_rates()
        };
        let cost = price_model_call(
            TokenUsage {
                tokens_in: 1_000_000,
                cache_read: 400_000,
                cache_write: 100_000,
                ..TokenUsage::default()
            },
            &rates,
        );
        // All 1,000,000 input tokens at $3, none carved out.
        assert_eq!(cost.cost_micros(), Some(3_000_000));
    }

    /// Same for reasoning: no reasoning rate means those tokens remain output.
    #[test]
    fn without_a_reasoning_rate_reasoning_tokens_are_billed_as_output() {
        let cost = price_model_call(
            TokenUsage {
                tokens_out: 1_000_000,
                reasoning: 250_000,
                ..TokenUsage::default()
            },
            &sonnet_rates(),
        );
        assert_eq!(cost.cost_micros(), Some(15_000_000));
    }

    /// Anthropic's published long-context band: past 200K prompt tokens the
    /// whole request bills at the higher rates.
    fn sonnet_with_long_context_band() -> ModelPricing {
        ModelPricing {
            base: sonnet_rates(),
            tiers: vec![RateTier {
                above_tokens: 200_000,
                rates: ModelRates {
                    input: rate_from_usd_per_million(6.0),
                    output: rate_from_usd_per_million(22.5),
                    cache_read: rate_from_usd_per_million(0.6),
                    cache_write: rate_from_usd_per_million(7.5),
                    reasoning: None,
                },
            }],
            service_tiers: Vec::new(),
        }
        .sorted()
    }

    /// Below the threshold nothing changes -- the overwhelming majority of
    /// calls must be untouched by band support existing.
    #[test]
    fn a_prompt_under_the_threshold_bills_at_the_headline_rate() {
        let cost = price_model_call_tiered(
            &sonnet_with_long_context_band(),
            TokenUsage {
                tokens_in: 100_000,
                ..TokenUsage::default()
            },
            None,
        );
        assert_eq!(cost.cost_micros(), Some(300_000));
    }

    /// The band replaces the rate for the WHOLE request. Billing only the
    /// overflow at the higher rate -- the marginal reading -- would charge
    /// 200,000 x $3 + 100,000 x $6 = $1.20 here, understating by a third.
    #[test]
    fn a_prompt_over_the_threshold_bills_every_token_at_the_band_rate() {
        let cost = price_model_call_tiered(
            &sonnet_with_long_context_band(),
            TokenUsage {
                tokens_in: 300_000,
                ..TokenUsage::default()
            },
            None,
        );
        assert_eq!(cost.cost_micros(), Some(1_800_000));
        assert_ne!(cost.cost_micros(), Some(1_200_000), "not marginal pricing");
    }

    /// Strictly above, matching the client: exactly at the threshold is still
    /// the headline rate.
    #[test]
    fn exactly_at_the_threshold_is_not_over_it() {
        let cost = price_model_call_tiered(
            &sonnet_with_long_context_band(),
            TokenUsage {
                tokens_in: 200_000,
                ..TokenUsage::default()
            },
            None,
        );
        assert_eq!(cost.cost_micros(), Some(600_000));
    }

    /// A request over 400K is also over 200K; the provider bills the band it
    /// lands in, so the highest cleared threshold wins regardless of the
    /// order the catalog listed them.
    #[test]
    fn the_highest_cleared_band_wins() {
        let pricing = ModelPricing {
            base: sonnet_rates(),
            tiers: vec![
                RateTier {
                    above_tokens: 400_000,
                    rates: ModelRates {
                        input: rate_from_usd_per_million(12.0),
                        ..ModelRates::default()
                    },
                },
                RateTier {
                    above_tokens: 200_000,
                    rates: ModelRates {
                        input: rate_from_usd_per_million(6.0),
                        ..ModelRates::default()
                    },
                },
            ],
            service_tiers: Vec::new(),
        }
        .sorted();
        let cost = price_model_call_tiered(
            &pricing,
            TokenUsage {
                tokens_in: 500_000,
                ..TokenUsage::default()
            },
            None,
        );
        assert_eq!(cost.cost_micros(), Some(6_000_000));
    }

    /// A band that quotes only `input` says nothing about cache reads. Those
    /// must keep the headline cache rate rather than falling out of pricing.
    #[test]
    fn a_band_overlays_only_the_components_it_publishes() {
        let pricing = ModelPricing {
            base: sonnet_rates(),
            tiers: vec![RateTier {
                above_tokens: 200_000,
                rates: ModelRates {
                    input: rate_from_usd_per_million(6.0),
                    ..ModelRates::default()
                },
            }],
            service_tiers: Vec::new(),
        }
        .sorted();
        let rates = resolve_rates(&pricing, 300_000, None);
        assert_eq!(rates.input, rate_from_usd_per_million(6.0));
        assert_eq!(rates.cache_read, rate_from_usd_per_million(0.3));
        assert_eq!(rates.output, rate_from_usd_per_million(15.0));
    }

    /// A fast lane is a different product, not a volume band, so it wins over
    /// the band where it publishes a rate.
    #[test]
    fn a_service_tier_wins_over_the_long_context_band() {
        let mut pricing = sonnet_with_long_context_band();
        pricing.service_tiers.push(ServiceTierRates {
            tier: "priority".into(),
            rates: ModelRates {
                input: rate_from_usd_per_million(15.0),
                ..ModelRates::default()
            },
        });
        let pricing = pricing.sorted();

        let rates = resolve_rates(&pricing, 300_000, Some("Priority"));
        // The lane sets input; the band still governs what the lane omits.
        assert_eq!(rates.input, rate_from_usd_per_million(15.0));
        assert_eq!(rates.output, rate_from_usd_per_million(22.5));
    }

    /// A tier the model does not publish must not silently reprice anything.
    #[test]
    fn an_unknown_service_tier_leaves_the_rates_alone() {
        let pricing = sonnet_with_long_context_band();
        let rates = resolve_rates(&pricing, 1_000, Some("no-such-lane"));
        assert_eq!(rates, sonnet_rates());
    }

    /// A model whose only rate lives in a band is priced, not unpriced.
    #[test]
    fn a_model_priced_only_by_a_band_is_not_empty() {
        let pricing = ModelPricing {
            base: ModelRates::default(),
            tiers: vec![RateTier {
                above_tokens: 200_000,
                rates: ModelRates {
                    input: rate_from_usd_per_million(6.0),
                    ..ModelRates::default()
                },
            }],
            service_tiers: Vec::new(),
        };
        assert!(!pricing.is_empty());
        // Under the threshold it has no applicable rate at all.
        assert_eq!(
            price_model_call_tiered(
                &pricing,
                TokenUsage {
                    tokens_in: 1_000,
                    ..TokenUsage::default()
                },
                None,
            ),
            PricedCost::Unpriced {
                reason: UnpricedReason::NoRates
            }
        );
    }

    /// Catalog order must not read as a price change: two documents listing
    /// the same bands differently have to compare equal.
    #[test]
    fn sorting_makes_catalog_order_irrelevant_to_equality() {
        let band_a = RateTier {
            above_tokens: 200_000,
            rates: ModelRates {
                input: rate_from_usd_per_million(6.0),
                ..ModelRates::default()
            },
        };
        let band_b = RateTier {
            above_tokens: 400_000,
            rates: ModelRates {
                input: rate_from_usd_per_million(12.0),
                ..ModelRates::default()
            },
        };
        let one = ModelPricing {
            base: sonnet_rates(),
            tiers: vec![band_a.clone(), band_b.clone()],
            service_tiers: Vec::new(),
        }
        .sorted();
        let other = ModelPricing {
            base: sonnet_rates(),
            tiers: vec![band_b, band_a],
            service_tiers: Vec::new(),
        }
        .sorted();
        assert_eq!(one, other);
    }

    /// The trap this whole design exists to avoid: pricing a summed usage is
    /// not pricing each call. Two 150K calls never clear a 200K band; their
    /// sum does.
    #[test]
    fn summing_before_pricing_would_invent_a_band_no_call_reached() {
        let pricing = sonnet_with_long_context_band();
        let call = TokenUsage {
            tokens_in: 150_000,
            ..TokenUsage::default()
        };
        let per_call: i64 = [call, call]
            .iter()
            .map(|usage| {
                price_model_call_tiered(&pricing, *usage, None)
                    .cost_micros()
                    .expect("priced")
            })
            .sum();
        let summed = price_model_call_tiered(
            &pricing,
            TokenUsage {
                tokens_in: 300_000,
                ..TokenUsage::default()
            },
            None,
        )
        .cost_micros()
        .expect("priced");

        assert_eq!(per_call, 900_000);
        assert_eq!(summed, 1_800_000);
        assert_ne!(per_call, summed, "aggregating first doubles the bill");
    }

    #[test]
    fn a_model_with_no_rates_is_unpriced_not_free() {
        let cost = price_model_call(
            TokenUsage {
                tokens_in: 1_000,
                tokens_out: 1_000,
                ..TokenUsage::default()
            },
            &ModelRates::default(),
        );
        assert_eq!(cost.cost_micros(), None);
        assert_eq!(cost.unpriced_reason(), Some(UnpricedReason::NoRates));
    }

    /// Rates exist but the event used no component they cover.
    #[test]
    fn rates_that_match_nothing_used_are_unpriced() {
        let rates = ModelRates {
            output: rate_from_usd_per_million(15.0),
            ..ModelRates::default()
        };
        let cost = price_model_call(
            TokenUsage {
                tokens_in: 1_000,
                ..TokenUsage::default()
            },
            &rates,
        );
        assert_eq!(
            cost.unpriced_reason(),
            Some(UnpricedReason::NoMatchingComponent)
        );
    }

    /// A partially priced model still prices what it can, matching EvoFlux
    /// (`if not components: return None`) rather than discarding the event.
    #[test]
    fn a_partially_priced_model_bills_the_components_it_covers() {
        let rates = ModelRates {
            input: rate_from_usd_per_million(3.0),
            ..ModelRates::default()
        };
        let cost = price_model_call(
            TokenUsage {
                tokens_in: 1_000_000,
                tokens_out: 1_000_000,
                ..TokenUsage::default()
            },
            &rates,
        );
        assert_eq!(cost.cost_micros(), Some(3_000_000));
    }

    /// Sub-micro amounts round rather than truncate to zero.
    #[test]
    fn rounds_half_up_instead_of_truncating() {
        let rates = ModelRates {
            input: rate_from_usd_per_million(3.0),
            ..ModelRates::default()
        };
        // 1 token at $3/M = 3 micro-USD exactly.
        assert_eq!(
            price_model_call(
                TokenUsage {
                    tokens_in: 1,
                    ..TokenUsage::default()
                },
                &rates
            )
            .cost_micros(),
            Some(3)
        );

        let tiny = ModelRates {
            input: rate_from_usd_per_million(0.4),
            ..ModelRates::default()
        };
        // 1 token at $0.40/M = 0.4 micro-USD, which must round to 0, not panic.
        assert_eq!(
            price_model_call(
                TokenUsage {
                    tokens_in: 1,
                    ..TokenUsage::default()
                },
                &tiny
            )
            .cost_micros(),
            Some(0)
        );
        // 2 tokens = 0.8 micro-USD, rounds up to 1.
        assert_eq!(
            price_model_call(
                TokenUsage {
                    tokens_in: 2,
                    ..TokenUsage::default()
                },
                &tiny
            )
            .cost_micros(),
            Some(1)
        );
    }

    /// Negative counters cannot appear on the wire, but a corrupt row must not
    /// produce a negative cost that silently offsets other members' spend.
    #[test]
    fn negative_counters_never_produce_a_negative_cost() {
        let cost = price_model_call(
            TokenUsage {
                tokens_in: -5_000,
                tokens_out: -1,
                cache_read: -10,
                cache_write: -10,
                reasoning: -10,
            },
            &sonnet_rates(),
        );
        assert_eq!(
            cost.unpriced_reason(),
            Some(UnpricedReason::NoMatchingComponent)
        );
    }

    /// The saturating i64 tokens a client can currently report must not
    /// overflow the accumulator into a negative or wrapped cost.
    #[test]
    fn absurd_token_counts_saturate_rather_than_wrap() {
        let cost = price_model_call(
            TokenUsage {
                tokens_in: i64::MAX,
                tokens_out: i64::MAX,
                ..TokenUsage::default()
            },
            &sonnet_rates(),
        );
        assert_eq!(cost.cost_micros(), Some(i64::MAX));
    }

    #[test]
    fn model_keys_normalize_case_and_padding() {
        assert_eq!(normalize_model_key("  OpenAI "), "openai");
        assert_eq!(normalize_model_key("Claude-Sonnet-4"), "claude-sonnet-4");
        assert_eq!(normalize_model_key(""), "");
    }

    #[test]
    fn unpriced_reason_round_trips_through_its_string_form() {
        for reason in [
            UnpricedReason::NoCatalog,
            UnpricedReason::ModelUnknown,
            UnpricedReason::NoRates,
            UnpricedReason::NoMatchingComponent,
        ] {
            assert_eq!(UnpricedReason::parse(reason.as_str()), Some(reason));
        }
        assert_eq!(UnpricedReason::parse("something_else"), None);
    }

    /// A model cost report needs to say where a total came from, so the
    /// component split has to sum back to exactly what `price_model_call`
    /// charges for the same usage.
    #[test]
    fn components_sum_to_the_same_total_price_model_call_reports() {
        let usage = TokenUsage {
            tokens_in: 300_000,
            tokens_out: 100_000,
            cache_read: 60_000,
            cache_write: 10_000,
            reasoning: 0,
        };
        let rates = sonnet_rates();
        let components = price_components(usage, &rates).expect("priced");
        let PricedCost::Priced { cost_micros } = price_model_call(usage, &rates) else {
            panic!("expected a priced result");
        };
        assert_eq!(components.total_usd_micros(), cost_micros);
        assert!(components.cache_read_usd_micros > 0);
        assert!(components.cache_write_usd_micros > 0);
        assert!(components.input_usd_micros > 0);
        assert!(components.output_usd_micros > 0);
    }

    /// Cache tokens fold into the input component, not their own bucket, when
    /// the model publishes no cache rate -- matching `price_model_call`.
    #[test]
    fn components_fold_cache_into_input_without_a_cache_rate() {
        let usage = TokenUsage {
            tokens_in: 100_000,
            tokens_out: 10_000,
            cache_read: 40_000,
            cache_write: 0,
            reasoning: 0,
        };
        let rates = ModelRates {
            input: rate_from_usd_per_million(3.0),
            output: rate_from_usd_per_million(15.0),
            cache_read: None,
            cache_write: None,
            reasoning: None,
        };
        let components = price_components(usage, &rates).expect("priced");
        assert_eq!(components.cache_read_usd_micros, 0);
        assert_eq!(components.cache_write_usd_micros, 0);
        // 100_000 tokens (cache is not carved out) at $3/M = 300_000 micros.
        assert_eq!(components.input_usd_micros, 300_000);
    }

    #[test]
    fn a_model_with_no_rates_has_no_components() {
        assert_eq!(
            price_components(
                TokenUsage {
                    tokens_in: 1000,
                    ..TokenUsage::default()
                },
                &ModelRates::default(),
            ),
            None
        );
    }

    /// Summing tokens across many events before pricing once must equal
    /// pricing each event and summing the totals -- the whole reason a report
    /// can price a project's traffic in one pass instead of one call per
    /// event.
    #[test]
    fn pricing_summed_usage_once_matches_pricing_each_event_and_summing() {
        let rates = sonnet_rates();
        let events = [
            TokenUsage {
                tokens_in: 120_000,
                tokens_out: 30_000,
                cache_read: 20_000,
                cache_write: 5_000,
                reasoning: 0,
            },
            TokenUsage {
                tokens_in: 80_000,
                tokens_out: 12_000,
                cache_read: 0,
                cache_write: 0,
                reasoning: 0,
            },
        ];
        let per_event_total: i64 = events
            .iter()
            .map(|usage| {
                price_model_call(*usage, &rates)
                    .cost_micros()
                    .expect("priced")
            })
            .sum();
        let summed_usage = TokenUsage {
            tokens_in: events.iter().map(|u| u.tokens_in).sum(),
            tokens_out: events.iter().map(|u| u.tokens_out).sum(),
            cache_read: events.iter().map(|u| u.cache_read).sum(),
            cache_write: events.iter().map(|u| u.cache_write).sum(),
            reasoning: events.iter().map(|u| u.reasoning).sum(),
        };
        let components = price_components(summed_usage, &rates).expect("priced");
        assert_eq!(components.total_usd_micros(), per_event_total);
    }
}
