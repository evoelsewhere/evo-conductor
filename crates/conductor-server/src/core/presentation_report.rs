//! Builds one `PresentationReport` by composing the same report builders
//! every other analytics view already uses -- see the module doc on
//! `conductor_domain::presentation_report` for why this exists as its own
//! composition step instead of being re-derived by each renderer.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use conductor_domain::{
    ModelCostReportTotals, OutlierTask, PresentationReport, ReportCallouts, ReportKind,
    ReportWindow, ResourceUsageScope, TopEntry,
};
use conductor_storage::repos::{CostReportFilters, ResourceUsageQuery};
use conductor_storage::Db;
use uuid::Uuid;

use crate::core::error::ApiResult;
use crate::core::model_pricing::SharedRateTable;

const TOP_N: usize = 5;
/// A task costing at least this many times its type's average is worth
/// calling out as a possible rework/scope-creep signal.
const OUTLIER_THRESHOLD: f64 = 2.0;

pub async fn build(
    db: &Db,
    rates: &SharedRateTable,
    project_id: Uuid,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    kinds: &[ReportKind],
) -> ApiResult<PresentationReport> {
    let window = ReportWindow::new(from, to);
    let project_name = db
        .instance()
        .get()
        .await?
        .map(|instance| instance.project_name)
        .unwrap_or_default();

    let current_query = usage_query(project_id, window.from, window.to);
    let previous_query = usage_query(project_id, window.previous_from, window.previous_to);
    let resource_usage = db.resource_usage();
    let (current, previous) = tokio::try_join!(
        resource_usage.analytics(&current_query),
        resource_usage.analytics(&previous_query),
    )?;

    let filters = CostReportFilters::default();
    let (member_rows, member_totals) = if kinds.contains(&ReportKind::Member) {
        let report =
            crate::core::member_cost_report::build(db, project_id, window.from, window.to, &filters)
                .await?;
        (report.rows, report.totals)
    } else {
        (Vec::new(), ModelCostReportTotals::default())
    };
    let (model_rows, model_totals) = if kinds.contains(&ReportKind::Model) {
        let report = crate::core::model_cost_report::build(
            db, rates, project_id, window.from, window.to, &filters,
        )
        .await?;
        (report.rows, report.totals)
    } else {
        (Vec::new(), ModelCostReportTotals::default())
    };
    let jira_rows = if kinds.contains(&ReportKind::Jira) {
        crate::core::task_cost_report::build(db, project_id, window.from, window.to)
            .await?
            .rows
    } else {
        Vec::new()
    };

    let top_members = top_entries(member_rows.iter().map(|row| {
        (
            row.display_name.clone(),
            row.total_cost_usd_micros,
            row.total_tokens,
        )
    }));
    let top_models = top_entries(model_rows.iter().map(|row| {
        (
            format!("{}:{}", row.provider, row.model),
            row.total_cost_usd_micros,
            row.total_tokens,
        )
    }));
    let top_tasks = top_entries(jira_rows.iter().map(|row| {
        (
            format!("{} — {}", row.issue_key, row.title),
            row.total_cost_usd_micros,
            row.total_tokens,
        )
    }));

    let callouts = ReportCallouts {
        top_members,
        top_models,
        top_tasks,
        cost_change_pct: change_pct(
            previous.totals.estimated_cost_usd_micros,
            current.totals.estimated_cost_usd_micros,
        ),
        cache_savings_usd_micros: current.totals.cache_savings_usd_micros,
        cache_savings_change_pct: change_pct(
            previous.totals.cache_savings_usd_micros,
            current.totals.cache_savings_usd_micros,
        ),
        unpriced_ratio_bps: ratio_bps(
            current.totals.unpriced_model_calls,
            current.totals.model_calls,
        ),
        cost_outlier_tasks: outlier_tasks(&jira_rows),
    };

    Ok(PresentationReport {
        project_name,
        window,
        kinds: kinds.to_vec(),
        totals: current.totals,
        daily: current.daily,
        members: member_rows,
        member_totals,
        models: model_rows,
        model_totals,
        jira_tasks: jira_rows,
        callouts,
    })
}

fn usage_query(project_id: Uuid, from: DateTime<Utc>, to: DateTime<Utc>) -> ResourceUsageQuery {
    ResourceUsageQuery {
        project_id,
        from,
        to,
        user_id: None,
        primary_role: None,
        resource_kind: None,
        resource_id: None,
        version_id: None,
        status: None,
        provider: None,
        model: None,
        installation_id: None,
        relation: None,
        scope: ResourceUsageScope::All,
        // The report only reads `totals`/`daily`; the activity page this
        // query type also supports is irrelevant here, so ask for as little
        // of it as the type allows rather than pretending to paginate it.
        limit: 1,
        offset: 0,
    }
}

/// Highest cost first, capped at `TOP_N` -- shared by every "top N by spend"
/// call-out so members/models/tasks are ranked identically.
fn top_entries(rows: impl Iterator<Item = (String, u64, u64)>) -> Vec<TopEntry> {
    let mut entries: Vec<TopEntry> = rows
        .map(|(label, total_cost_usd_micros, total_tokens)| TopEntry {
            label,
            total_cost_usd_micros,
            total_tokens,
        })
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.total_cost_usd_micros));
    entries.truncate(TOP_N);
    entries
}

/// `None` when `previous` is zero: a ratio against zero has no honest
/// percentage to report, not a 0% or infinite change.
fn change_pct(previous: u64, current: u64) -> Option<f64> {
    if previous == 0 {
        return None;
    }
    Some((current as f64 - previous as f64) / previous as f64 * 100.0)
}

fn ratio_bps(part: u64, whole: u64) -> u32 {
    if whole == 0 {
        return 0;
    }
    ((part as u128 * 10_000) / whole as u128) as u32
}

fn outlier_tasks(rows: &[conductor_domain::TaskCostRow]) -> Vec<OutlierTask> {
    let mut totals_by_type: HashMap<&str, (u64, u64)> = HashMap::new();
    for row in rows {
        if !row.precise || row.total_cost_usd_micros == 0 {
            continue;
        }
        let entry = totals_by_type.entry(row.resolved_type.as_str()).or_default();
        entry.0 += row.total_cost_usd_micros;
        entry.1 += 1;
    }

    let mut outliers: Vec<OutlierTask> = rows
        .iter()
        .filter(|row| row.precise && row.total_cost_usd_micros > 0)
        .filter_map(|row| {
            let (total, count) = totals_by_type.get(row.resolved_type.as_str())?;
            if *count < 2 {
                // A type with only this one task has no siblings to compare
                // against -- not an outlier, just the only data point.
                return None;
            }
            // The mean of this task's siblings, excluding itself -- otherwise
            // a task would always be compared partly against its own cost,
            // understating how far above its siblings it really sits.
            let sibling_total = total - row.total_cost_usd_micros;
            let sibling_count = count - 1;
            let mean = sibling_total as f64 / sibling_count as f64;
            if mean <= 0.0 {
                return None;
            }
            let ratio = row.total_cost_usd_micros as f64 / mean;
            (ratio >= OUTLIER_THRESHOLD).then(|| OutlierTask {
                issue_key: row.issue_key.clone(),
                title: row.title.clone(),
                resolved_type: row.resolved_type.clone(),
                total_cost_usd_micros: row.total_cost_usd_micros,
                times_the_type_average: ratio,
            })
        })
        .collect();
    outliers.sort_by(|a, b| b.times_the_type_average.total_cmp(&a.times_the_type_average));
    outliers.truncate(TOP_N);
    outliers
}

#[cfg(test)]
mod tests {
    use super::*;
    use conductor_domain::TaskCostRow;

    fn task(issue_key: &str, resolved_type: &str, cost: u64, precise: bool) -> TaskCostRow {
        TaskCostRow {
            issue_key: issue_key.to_string(),
            title: format!("Task {issue_key}"),
            resolved_type: resolved_type.to_string(),
            resolved_project: None,
            status: "In Progress".to_string(),
            parent_key: None,
            assignee_display_name: None,
            matched_user_id: None,
            matched: true,
            precise,
            calls: 1,
            total_tokens: 1_000,
            total_cost_usd_micros: cost,
            cache_savings_usd_micros: 0,
            tokens_in: 1_000,
            tokens_out: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            total_duration_ms: 0,
            models: Vec::new(),
            by_status: Vec::new(),
            rollup_by_status: Vec::new(),
        }
    }

    #[test]
    fn top_entries_ranks_by_cost_and_caps_at_five() {
        let rows = (0..8).map(|i| (format!("row-{i}"), (i as u64) * 100, 10));
        let entries = top_entries(rows);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].label, "row-7");
        assert_eq!(entries[0].total_cost_usd_micros, 700);
        assert_eq!(entries[4].label, "row-3");
    }

    #[test]
    fn change_pct_is_none_against_a_zero_previous_period() {
        assert_eq!(change_pct(0, 500), None);
    }

    #[test]
    fn change_pct_reports_signed_percentage_change() {
        assert_eq!(change_pct(100, 150), Some(50.0));
        assert_eq!(change_pct(100, 50), Some(-50.0));
    }

    #[test]
    fn ratio_bps_is_zero_when_nothing_happened() {
        assert_eq!(ratio_bps(0, 0), 0);
    }

    #[test]
    fn ratio_bps_computes_basis_points() {
        // 25 of 200 is 12.5% = 1250 basis points.
        assert_eq!(ratio_bps(25, 200), 1250);
    }

    #[test]
    fn outlier_tasks_flags_a_task_well_above_its_type_siblings() {
        let rows = vec![
            task("A-1", "task", 1_000_000, true),
            task("A-2", "task", 1_100_000, true),
            task("A-3", "task", 5_000_000, true), // ~4.8x its siblings' mean
            task("A-4", "bug", 200_000, true),
        ];
        let outliers = outlier_tasks(&rows);
        assert_eq!(outliers.len(), 1);
        assert_eq!(outliers[0].issue_key, "A-3");
        assert!(outliers[0].times_the_type_average > OUTLIER_THRESHOLD);
    }

    #[test]
    fn outlier_tasks_ignores_a_type_with_no_siblings_to_compare() {
        let rows = vec![
            task("B-1", "epic", 9_000_000, true),
            task("B-2", "task", 100_000, true),
        ];
        assert!(outlier_tasks(&rows).is_empty());
    }

    #[test]
    fn outlier_tasks_ignores_fallback_rows_that_are_not_precise() {
        let rows = vec![
            task("C-1", "task", 100_000, true),
            task("C-2", "task", 100_000, true),
            task("C-3", "task", 9_000_000, false),
        ];
        assert!(outlier_tasks(&rows).is_empty());
    }
}
