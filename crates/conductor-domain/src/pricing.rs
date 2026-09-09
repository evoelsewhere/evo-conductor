//! Model-call cost estimation — a straight port of EvoFlux's own pricing
//! algorithm (`app/agent/usage.py` in the evoflux repository), so a
//! Conductor-computed price and an EvoFlux-computed price agree whenever
//! they price the same call from the same catalog data.
//!
//! Pure, no I/O: catalog fetch/caching lives in `conductor-server`, this
//! module only turns already-resolved rates and token counts into USD.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Per-token rates in USD per one million tokens. `None` means the catalog
/// does not state that rate, not that it is free.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelCostRates {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
    pub reasoning: Option<f64>,
}

/// A rate override that replaces the headline rates once the request's
/// prompt token count passes `above_tokens` — e.g. a long-context surcharge.
/// Only the fields the tier actually overrides are `Some`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CostTier {
    pub above_tokens: u64,
    pub rates: ModelCostRates,
}

/// One catalog entry for a `(provider, model)` pair.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelPricing {
    pub base: ModelCostRates,
    pub tiers: Vec<CostTier>,
}

/// One row of the world AI pricing catalog, for display rather than billing:
/// which model, and what it costs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPricingCatalogEntry {
    pub provider: String,
    pub model: String,
    pub pricing: ModelPricing,
}

/// The full catalog Conductor knows about right now, for a reference-price
/// screen rather than a usage/spend report (see `ResourceUsageAnalytics`
/// for that).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelPricingCatalogSnapshot {
    pub entries: Vec<ModelPricingCatalogEntry>,
    /// `None` before the catalog has ever been fetched or read from disk
    /// cache (fresh install, first request, no network yet).
    pub fetched_at: Option<DateTime<Utc>>,
}

/// The rates that actually apply to one request, after resolving any
/// long-context tier.
///
/// Mirrors `_tiered_rates` in `app/agent/usage.py`: the highest applicable
/// threshold wins (a request over 400K is also over 200K, and the provider
/// bills the band it lands in), and a tier only overrides the fields it
/// states, leaving the rest at the headline rate.
pub fn resolve_rates(pricing: &ModelPricing, prompt_tokens: u64) -> ModelCostRates {
    let mut rates = pricing.base;
    let applicable = pricing
        .tiers
        .iter()
        .filter(|tier| prompt_tokens > tier.above_tokens)
        .max_by_key(|tier| tier.above_tokens);
    if let Some(tier) = applicable {
        if tier.rates.input.is_some() {
            rates.input = tier.rates.input;
        }
        if tier.rates.output.is_some() {
            rates.output = tier.rates.output;
        }
        if tier.rates.cache_read.is_some() {
            rates.cache_read = tier.rates.cache_read;
        }
        if tier.rates.cache_write.is_some() {
            rates.cache_write = tier.rates.cache_write;
        }
        if tier.rates.reasoning.is_some() {
            rates.reasoning = tier.rates.reasoning;
        }
    }
    rates
}

/// Price a token total from already-resolved rates, in USD.
///
/// Mirrors `estimate_cost` in `app/agent/usage.py`: cache-read and
/// cache-write tokens are each billed at their own rate and subtracted from
/// the plain input count first, and reasoning tokens are billed at their
/// own rate and subtracted from the completion count first, so none of them
/// is double-counted at the headline input/output rate. Returns `None` when
/// no rate applied at all (nothing priced), matching the Python function's
/// contract.
#[allow(clippy::too_many_arguments)]
pub fn estimate_cost_usd(
    rates: &ModelCostRates,
    tokens_in: u64,
    tokens_out: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
) -> Option<f64> {
    const PER_MILLION: f64 = 1_000_000.0;
    let mut remaining_input = tokens_in;
    let mut remaining_output = tokens_out;
    let mut total = 0.0_f64;
    let mut priced_any = false;

    if let Some(rate) = rates.cache_read {
        if cache_read_tokens > 0 {
            total += cache_read_tokens as f64 * rate / PER_MILLION;
            priced_any = true;
            remaining_input = remaining_input.saturating_sub(cache_read_tokens);
        }
    }

    if let Some(rate) = rates.cache_write {
        if cache_write_tokens > 0 {
            total += cache_write_tokens as f64 * rate / PER_MILLION;
            priced_any = true;
            remaining_input = remaining_input.saturating_sub(cache_write_tokens);
        }
    }

    if let Some(rate) = rates.input {
        if remaining_input > 0 {
            total += remaining_input as f64 * rate / PER_MILLION;
            priced_any = true;
        }
    }

    if let Some(rate) = rates.reasoning {
        if reasoning_tokens > 0 {
            total += reasoning_tokens as f64 * rate / PER_MILLION;
            priced_any = true;
            remaining_output = remaining_output.saturating_sub(reasoning_tokens);
        }
    }

    if let Some(rate) = rates.output {
        if remaining_output > 0 {
            total += remaining_output as f64 * rate / PER_MILLION;
            priced_any = true;
        }
    }

    priced_any.then_some(total)
}

/// Net USD saved (or lost) by billing cache tokens at their own rates
/// instead of the plain input rate: the discount earned on `cache_read`
/// tokens minus the premium paid on `cache_write` tokens. `None` when the
/// catalog states no `input` rate — there is no baseline to compare
/// against, not "zero savings". A rate the catalog does not state for one
/// side (say, no `cache_write` rate at all) contributes nothing to the
/// total, since `estimate_cost_usd` would have billed those tokens at the
/// plain input rate anyway — no premium, no discount, no difference.
pub fn cache_savings_usd(
    rates: &ModelCostRates,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
) -> Option<f64> {
    const PER_MILLION: f64 = 1_000_000.0;
    let input_rate = rates.input?;
    let mut savings = 0.0_f64;
    if let Some(read_rate) = rates.cache_read {
        savings += cache_read_tokens as f64 * (input_rate - read_rate) / PER_MILLION;
    }
    if let Some(write_rate) = rates.cache_write {
        savings -= cache_write_tokens as f64 * (write_rate - input_rate) / PER_MILLION;
    }
    Some(savings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rates(input: f64, output: f64) -> ModelCostRates {
        ModelCostRates {
            input: Some(input),
            output: Some(output),
            ..Default::default()
        }
    }

    #[test]
    fn prices_plain_input_and_output() {
        let rates = rates(3.0, 15.0);
        let cost = estimate_cost_usd(&rates, 1_000_000, 1_000_000, 0, 0, 0).unwrap();
        assert!((cost - 18.0).abs() < 1e-9);
    }

    #[test]
    fn no_rates_returns_none() {
        let rates = ModelCostRates::default();
        assert_eq!(estimate_cost_usd(&rates, 1_000, 1_000, 0, 0, 0), None);
    }

    #[test]
    fn cache_read_tokens_are_billed_separately_and_not_double_counted() {
        let rates = ModelCostRates {
            input: Some(3.0),
            cache_read: Some(0.3),
            ..Default::default()
        };
        // 1,000,000 input tokens, 400,000 of them cache reads.
        let cost = estimate_cost_usd(&rates, 1_000_000, 0, 400_000, 0, 0).unwrap();
        let expected = 400_000.0 * 0.3 / 1_000_000.0 + 600_000.0 * 3.0 / 1_000_000.0;
        assert!((cost - expected).abs() < 1e-9);
    }

    #[test]
    fn cache_write_tokens_are_billed_separately_and_not_double_counted() {
        let rates = ModelCostRates {
            input: Some(3.0),
            cache_write: Some(3.75),
            ..Default::default()
        };
        // 1,000,000 input tokens, 250,000 of them cache writes.
        let cost = estimate_cost_usd(&rates, 1_000_000, 0, 0, 250_000, 0).unwrap();
        let expected = 250_000.0 * 3.75 / 1_000_000.0 + 750_000.0 * 3.0 / 1_000_000.0;
        assert!((cost - expected).abs() < 1e-9);
    }

    #[test]
    fn reasoning_tokens_are_billed_separately_and_not_double_counted() {
        let rates = ModelCostRates {
            output: Some(15.0),
            reasoning: Some(60.0),
            ..Default::default()
        };
        let cost = estimate_cost_usd(&rates, 0, 1_000_000, 0, 0, 200_000).unwrap();
        let expected = 200_000.0 * 60.0 / 1_000_000.0 + 800_000.0 * 15.0 / 1_000_000.0;
        assert!((cost - expected).abs() < 1e-9);
    }

    #[test]
    fn highest_applicable_tier_wins() {
        let pricing = ModelPricing {
            base: rates(3.0, 15.0),
            tiers: vec![
                CostTier {
                    above_tokens: 200_000,
                    rates: rates(6.0, 22.5),
                },
                CostTier {
                    above_tokens: 400_000,
                    rates: rates(9.0, 30.0),
                },
            ],
        };
        let resolved = resolve_rates(&pricing, 500_000);
        assert_eq!(resolved.input, Some(9.0));
        assert_eq!(resolved.output, Some(30.0));
    }

    #[test]
    fn tier_only_overrides_fields_it_states() {
        let pricing = ModelPricing {
            base: ModelCostRates {
                input: Some(3.0),
                output: Some(15.0),
                cache_read: Some(0.3),
                ..Default::default()
            },
            tiers: vec![CostTier {
                above_tokens: 200_000,
                rates: ModelCostRates {
                    input: Some(6.0),
                    ..Default::default()
                },
            }],
        };
        let resolved = resolve_rates(&pricing, 300_000);
        assert_eq!(resolved.input, Some(6.0));
        // Untouched by the tier, so still the headline rate.
        assert_eq!(resolved.output, Some(15.0));
        assert_eq!(resolved.cache_read, Some(0.3));
    }

    #[test]
    fn tier_at_or_below_threshold_does_not_apply() {
        let pricing = ModelPricing {
            base: rates(3.0, 15.0),
            tiers: vec![CostTier {
                above_tokens: 200_000,
                rates: rates(6.0, 22.5),
            }],
        };
        let resolved = resolve_rates(&pricing, 200_000);
        assert_eq!(resolved.input, Some(3.0));
    }

    #[test]
    fn no_input_rate_means_no_savings_baseline() {
        let rates = ModelCostRates {
            cache_read: Some(0.3),
            ..Default::default()
        };
        assert_eq!(cache_savings_usd(&rates, 1_000_000, 0), None);
    }

    #[test]
    fn cache_read_discount_is_a_positive_saving() {
        let rates = ModelCostRates {
            input: Some(3.0),
            cache_read: Some(0.3),
            ..Default::default()
        };
        let savings = cache_savings_usd(&rates, 1_000_000, 0).unwrap();
        let expected = 1_000_000.0 * (3.0 - 0.3) / 1_000_000.0;
        assert!((savings - expected).abs() < 1e-9);
    }

    #[test]
    fn cache_write_premium_is_a_net_loss() {
        let rates = ModelCostRates {
            input: Some(3.0),
            cache_write: Some(3.75),
            ..Default::default()
        };
        let savings = cache_savings_usd(&rates, 0, 1_000_000).unwrap();
        let expected = -(1_000_000.0 * (3.75 - 3.0) / 1_000_000.0);
        assert!(savings < 0.0);
        assert!((savings - expected).abs() < 1e-9);
    }

    #[test]
    fn read_and_write_net_against_each_other() {
        let rates = ModelCostRates {
            input: Some(3.0),
            cache_read: Some(0.3),
            cache_write: Some(3.75),
            ..Default::default()
        };
        let savings = cache_savings_usd(&rates, 1_000_000, 1_000_000).unwrap();
        let expected =
            1_000_000.0 * (3.0 - 0.3) / 1_000_000.0 - 1_000_000.0 * (3.75 - 3.0) / 1_000_000.0;
        assert!((savings - expected).abs() < 1e-9);
    }

    #[test]
    fn no_cache_activity_is_zero_savings() {
        let rates = ModelCostRates {
            input: Some(3.0),
            cache_read: Some(0.3),
            cache_write: Some(3.75),
            ..Default::default()
        };
        assert_eq!(cache_savings_usd(&rates, 0, 0), Some(0.0));
    }

    #[test]
    fn missing_side_rate_contributes_nothing() {
        // cache_write tokens present but the catalog states no cache_write
        // rate: those tokens would have billed at the plain input rate
        // anyway, so they change nothing.
        let rates = ModelCostRates {
            input: Some(3.0),
            cache_read: Some(0.3),
            ..Default::default()
        };
        let savings = cache_savings_usd(&rates, 1_000_000, 500_000).unwrap();
        let expected = 1_000_000.0 * (3.0 - 0.3) / 1_000_000.0;
        assert!((savings - expected).abs() < 1e-9);
    }
}
