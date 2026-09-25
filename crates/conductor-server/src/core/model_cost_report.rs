//! Project-wide spend broken down by model, and by the token components a
//! models.dev rate prices separately (input, output, cache read, cache
//! write).
//!
//! Every other cost view in Conductor answers "how much" per member or per
//! resource. This one answers "where the money actually goes": which model,
//! and which part of a call -- a project burning most of its spend on cache
//! writes needs a different fix than one burning it on output tokens, and
//! nothing else in the product can tell those apart.

use chrono::{DateTime, Utc};
use conductor_domain::{
    price_components, CostComponents, ModelCostReport, ModelCostReportRow, ModelCostReportTotals,
    TokenUsage,
};
use conductor_storage::repos::{CostReportFilters, RawModelCostRow};
use conductor_storage::Db;
use uuid::Uuid;

use crate::core::model_pricing::{resolve_provider_alias, RateTable, SharedRateTable};

pub async fn build(
    db: &Db,
    rates: &SharedRateTable,
    project_id: Uuid,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    filters: &CostReportFilters,
) -> Result<ModelCostReport, sqlx::Error> {
    let raw = db
        .telemetry()
        .model_cost_totals(project_id, from, to, filters)
        .await?;
    let snapshot = rates.snapshot();

    let rows: Vec<ModelCostReportRow> = raw
        .into_iter()
        .map(|row| split_components(row, &snapshot))
        .collect();

    let rows = merge_alias_duplicates(rows);

    // The storage query already orders by cost, but component splitting must
    // not be allowed to reorder it -- a caller expecting "highest spend
    // first" would silently get wrong results.
    let rows = sort_by_cost_desc(rows);

    let totals = rows
        .iter()
        .fold(ModelCostReportTotals::default(), |mut acc, row| {
            acc.calls += row.calls;
            acc.unpriced_calls += row.unpriced_calls;
            acc.input_tokens += row.input_tokens;
            acc.output_tokens += row.output_tokens;
            acc.cache_read_tokens += row.cache_read_tokens;
            acc.cache_write_tokens += row.cache_write_tokens;
            acc.reasoning_tokens += row.reasoning_tokens;
            acc.total_tokens += row.total_tokens;
            acc.total_cost_usd_micros += row.total_cost_usd_micros;
            acc.cache_savings_usd_micros += row.cache_savings_usd_micros;
            acc
        });

    Ok(ModelCostReport {
        from,
        to,
        rows,
        totals,
    })
}

/// Highest spend first. A separate, tested function rather than an inline
/// `sort_by` so component splitting -- which touches every row -- cannot
/// quietly change an ordering the caller depends on.
fn sort_by_cost_desc(mut rows: Vec<ModelCostReportRow>) -> Vec<ModelCostReportRow> {
    rows.sort_by_key(|row| std::cmp::Reverse(row.total_cost_usd_micros));
    rows
}

/// Merge rows whose provider was an alias for the same canonical provider.
///
/// `model_cost_totals` groups by the raw `provider` stored on the event, so
/// `googlegenai` and `google` would otherwise appear as two separate lines
/// even though they resolve to the same rate card. This collapses them into
/// one, keeping the canonical provider name.
fn merge_alias_duplicates(rows: Vec<ModelCostReportRow>) -> Vec<ModelCostReportRow> {
    use std::collections::HashMap;

    let mut merged: HashMap<(String, String), ModelCostReportRow> = HashMap::new();
    for row in rows {
        let key = (resolve_provider_alias(&row.provider).to_string(), row.model.clone());
        if let Some(existing) = merged.get_mut(&key) {
            existing.calls += row.calls;
            existing.unpriced_calls += row.unpriced_calls;
            existing.input_tokens += row.input_tokens;
            existing.output_tokens += row.output_tokens;
            existing.cache_read_tokens += row.cache_read_tokens;
            existing.cache_write_tokens += row.cache_write_tokens;
            existing.reasoning_tokens += row.reasoning_tokens;
            existing.total_tokens += row.total_tokens;
            existing.input_cost_usd_micros += row.input_cost_usd_micros;
            existing.output_cost_usd_micros += row.output_cost_usd_micros;
            existing.cache_read_cost_usd_micros += row.cache_read_cost_usd_micros;
            existing.cache_write_cost_usd_micros += row.cache_write_cost_usd_micros;
            existing.total_cost_usd_micros += row.total_cost_usd_micros;
            existing.cache_savings_usd_micros += row.cache_savings_usd_micros;
        } else {
            let mut row = row;
            row.provider = resolve_provider_alias(&row.provider).to_string();
            merged.insert(key, row);
        }
    }
    let mut rows: Vec<_> = merged.into_values().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.total_cost_usd_micros));
    rows
}

/// Split one model's stored total into components, using the model's current
/// rate as the split ratio.
///
/// The headline `total_cost_usd_micros` is always the value storage already
/// summed (the same figure every other report shows for this model and
/// window); the four components are scaled so they add up to it exactly,
/// rather than computed independently and left to disagree with it. That
/// scaling is also what keeps a row honest when Conductor has no current rate
/// for the model at all -- every component is then zero, and the whole total
/// shows up as "unpriced" rather than silently guessing a split.
fn split_components(row: RawModelCostRow, rates: &RateTable) -> ModelCostReportRow {
    let total_tokens = row
        .tokens_in
        .saturating_add(row.tokens_out)
        .saturating_add(row.reasoning_tokens);
    let avg_usd_micros_per_million_tokens = if total_tokens == 0 {
        0
    } else {
        ((row.total_cost_usd_micros as u128 * 1_000_000) / total_tokens as u128) as u64
    };

    let usage = TokenUsage {
        tokens_in: row.tokens_in as i64,
        tokens_out: row.tokens_out as i64,
        cache_read: row.cache_read_tokens as i64,
        cache_write: row.cache_write_tokens as i64,
        reasoning: row.reasoning_tokens as i64,
    };
    let current_rate = rates.current_rate_for(&row.provider, &row.model);
    let components = current_rate.and_then(|rate| price_components(usage, &rate));

    let (input, output, cache_read, cache_write) = match components {
        Some(components) if components.total_usd_micros() > 0 && row.total_cost_usd_micros > 0 => {
            scale_to_total(&components, row.total_cost_usd_micros)
        }
        // No current rate, a zero split, or a zero stored total: nothing to
        // proportion, so the whole amount is unattributed rather than an
        // invented split.
        _ => (0, 0, 0, 0),
    };

    ModelCostReportRow {
        provider: row.provider,
        model: row.model,
        calls: row.calls,
        unpriced_calls: row.unpriced_calls,
        input_tokens: row.tokens_in,
        output_tokens: row.tokens_out,
        cache_read_tokens: row.cache_read_tokens,
        cache_write_tokens: row.cache_write_tokens,
        reasoning_tokens: row.reasoning_tokens,
        total_tokens,
        input_cost_usd_micros: input,
        output_cost_usd_micros: output,
        cache_read_cost_usd_micros: cache_read,
        cache_write_cost_usd_micros: cache_write,
        total_cost_usd_micros: row.total_cost_usd_micros,
        cache_savings_usd_micros: row.cache_savings_usd_micros,
        avg_usd_micros_per_million_tokens,
    }
}

/// Scale four component amounts so they sum to exactly `total`, keeping their
/// current-rate proportions. The last component absorbs whatever rounding
/// leaves over, so the four always add up to `total` bit for bit -- a report
/// whose parts do not sum to its own headline number reads as broken.
fn scale_to_total(components: &CostComponents, total: u64) -> (u64, u64, u64, u64) {
    let basis = components.total_usd_micros().max(1) as u128;
    let total = total as u128;
    let scale = |value: i64| -> u64 { ((value.max(0) as u128 * total) / basis) as u64 };

    let input = scale(components.input_usd_micros);
    let output = scale(components.output_usd_micros);
    let cache_read = scale(components.cache_read_usd_micros);
    let mut cache_write = scale(components.cache_write_usd_micros);
    // Absorb the remainder from integer-division truncation in the largest,
    // most-visible bucket's neighbour rather than silently dropping it.
    let assigned = input + output + cache_read + cache_write;
    if let Some(remainder) = (total as u64).checked_sub(assigned) {
        cache_write += remainder;
    }
    (input, output, cache_read, cache_write)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conductor_domain::ModelRates;

    fn raw(
        provider: &str,
        model: &str,
        calls: u64,
        unpriced: u64,
        total_cost: u64,
    ) -> RawModelCostRow {
        RawModelCostRow {
            provider: provider.to_string(),
            model: model.to_string(),
            calls,
            unpriced_calls: unpriced,
            tokens_in: 1_000_000,
            tokens_out: 200_000,
            cache_read_tokens: 100_000,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            total_cost_usd_micros: total_cost,
            cache_savings_usd_micros: 0,
        }
    }

    /// The whole point of scaling: the four components must always sum to
    /// the same headline total the rest of the app already shows, no matter
    /// what the current rate card happens to be.
    #[test]
    fn components_always_sum_to_the_stored_total() {
        let table = RateTable::for_test([(
            "anthropic",
            "claude-sonnet-4",
            ModelRates {
                input: Some(3_000_000),
                output: Some(15_000_000),
                cache_read: Some(300_000),
                cache_write: None,
                reasoning: None,
            },
        )]);
        let row = split_components(raw("anthropic", "claude-sonnet-4", 5, 0, 12_345), &table);
        assert_eq!(
            row.input_cost_usd_micros
                + row.output_cost_usd_micros
                + row.cache_read_cost_usd_micros
                + row.cache_write_cost_usd_micros,
            row.total_cost_usd_micros
        );
        assert_eq!(row.total_cost_usd_micros, 12_345);
    }

    /// A model with no current rate cannot honestly claim a split: the whole
    /// total surfaces as unattributed rather than guessed at.
    #[test]
    fn a_model_with_no_current_rate_reports_the_total_with_no_split() {
        let table = RateTable::default();
        let row = split_components(raw("mystery", "ghost-model", 3, 3, 4_200), &table);
        assert_eq!(row.total_cost_usd_micros, 4_200);
        assert_eq!(row.input_cost_usd_micros, 0);
        assert_eq!(row.output_cost_usd_micros, 0);
        assert_eq!(row.cache_read_cost_usd_micros, 0);
        assert_eq!(row.cache_write_cost_usd_micros, 0);
    }

    #[test]
    fn avg_cost_per_million_tokens_is_blended_and_zero_when_there_are_no_tokens() {
        let table = RateTable::default();
        let priced = split_components(raw("p", "m", 1, 0, 2_000_000), &table);
        // 1_200_000 total tokens at 2_000_000 micros => ~1_666_666 micros/M.
        assert_eq!(priced.avg_usd_micros_per_million_tokens, 1_666_666);

        let mut zero_tokens = raw("p", "m", 1, 0, 500);
        zero_tokens.tokens_in = 0;
        zero_tokens.tokens_out = 0;
        zero_tokens.reasoning_tokens = 0;
        let row = split_components(zero_tokens, &table);
        assert_eq!(row.avg_usd_micros_per_million_tokens, 0);
    }

    /// Splitting components must not reorder what storage already sorted by
    /// cost -- exercised as its own function since `build` needs a database.
    #[test]
    fn sort_by_cost_desc_puts_the_highest_spend_first() {
        let row = |model: &str, total: u64| ModelCostReportRow {
            model: model.into(),
            total_cost_usd_micros: total,
            ..ModelCostReportRow::default()
        };
        let rows = sort_by_cost_desc(vec![row("a", 10), row("c", 999), row("b", 100)]);
        let order: Vec<&str> = rows.iter().map(|r| r.model.as_str()).collect();
        assert_eq!(order, ["c", "b", "a"]);
    }
}
