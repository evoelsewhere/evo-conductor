//! Project-wide spend broken down by member.
//!
//! The model cost report answers "where the money goes"; this answers "who
//! is spending it". A member's calls can span many models with different
//! rates, so unlike the model report there is no per-component cost split
//! here -- only token components and the one authoritative total per member.

use chrono::{DateTime, Utc};
use conductor_domain::{MemberCostReport, MemberCostReportRow, ModelCostReportTotals};
use conductor_storage::repos::{CostReportFilters, RawMemberCostRow};
use conductor_storage::Db;
use uuid::Uuid;

pub async fn build(
    db: &Db,
    project_id: Uuid,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    filters: &CostReportFilters,
) -> Result<MemberCostReport, sqlx::Error> {
    let raw = db
        .telemetry()
        .member_cost_totals(project_id, from, to, filters)
        .await?;

    let rows: Vec<MemberCostReportRow> = raw.into_iter().map(to_row).collect();
    // Storage already orders by cost; keep it explicit and testable rather
    // than relying on the query never changing.
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
            acc
        });

    Ok(MemberCostReport {
        from,
        to,
        rows,
        totals,
    })
}

fn to_row(row: RawMemberCostRow) -> MemberCostReportRow {
    let total_tokens = row
        .tokens_in
        .saturating_add(row.tokens_out)
        .saturating_add(row.reasoning_tokens);
    let avg_usd_micros_per_million_tokens = if total_tokens == 0 {
        0
    } else {
        ((row.total_cost_usd_micros as u128 * 1_000_000) / total_tokens as u128) as u64
    };

    MemberCostReportRow {
        user_id: row.user_id,
        display_name: row.display_name,
        email: row.email,
        primary_role: row.primary_role,
        calls: row.calls,
        unpriced_calls: row.unpriced_calls,
        input_tokens: row.tokens_in,
        output_tokens: row.tokens_out,
        cache_read_tokens: row.cache_read_tokens,
        cache_write_tokens: row.cache_write_tokens,
        reasoning_tokens: row.reasoning_tokens,
        total_tokens,
        total_cost_usd_micros: row.total_cost_usd_micros,
        avg_usd_micros_per_million_tokens,
    }
}

/// Highest spend first, exercised on its own since `build` needs a database.
fn sort_by_cost_desc(mut rows: Vec<MemberCostReportRow>) -> Vec<MemberCostReportRow> {
    rows.sort_by_key(|row| std::cmp::Reverse(row.total_cost_usd_micros));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use conductor_domain::PrimaryRole;

    fn raw(name: &str, total_cost: u64) -> RawMemberCostRow {
        RawMemberCostRow {
            user_id: Uuid::new_v4(),
            display_name: name.to_string(),
            email: format!("{name}@example.test"),
            primary_role: PrimaryRole::User,
            calls: 1,
            unpriced_calls: 0,
            tokens_in: 1_000_000,
            tokens_out: 200_000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            total_cost_usd_micros: total_cost,
        }
    }

    #[test]
    fn avg_cost_per_million_tokens_is_blended_and_zero_when_there_are_no_tokens() {
        let row = to_row(raw("alice", 2_000_000));
        // 1_200_000 tokens at 2_000_000 micros => ~1_666_666 micros/M.
        assert_eq!(row.avg_usd_micros_per_million_tokens, 1_666_666);

        let mut zero_tokens = raw("bob", 500);
        zero_tokens.tokens_in = 0;
        zero_tokens.tokens_out = 0;
        let row = to_row(zero_tokens);
        assert_eq!(row.avg_usd_micros_per_million_tokens, 0);
    }

    #[test]
    fn sort_by_cost_desc_puts_the_highest_spend_first() {
        let rows = vec![
            to_row(raw("alice", 10)),
            to_row(raw("carol", 999)),
            to_row(raw("bob", 100)),
        ];
        let sorted = sort_by_cost_desc(rows);
        let order: Vec<&str> = sorted.iter().map(|r| r.display_name.as_str()).collect();
        assert_eq!(order, ["carol", "bob", "alice"]);
    }
}
