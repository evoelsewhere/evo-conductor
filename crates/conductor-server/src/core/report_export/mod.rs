//! Renders a `conductor_domain::PresentationReport` into a real Office file
//! -- `.xlsx` with native Excel charts, `.pptx` with a fixed slide template.

pub mod pptx;
pub mod xlsx;

pub use pptx::render_pptx;
pub use xlsx::render_xlsx;

#[cfg(test)]
pub(crate) mod fixtures {
    use chrono::{Duration, Utc};
    use conductor_domain::{
        MemberCostReportRow, ModelCostReportRow, ModelCostReportTotals, OutlierTask,
        PresentationReport, PrimaryRole, ReportCallouts, ReportKind, ReportWindow,
        ResourceUsageDay, ResourceUsageTotals, TaskCostRow, TopEntry,
    };
    use uuid::Uuid;

    /// A small but non-empty report exercising every sheet/slide -- enough
    /// to prove a renderer produces a well-formed file, not a claim about
    /// realistic numbers.
    pub(crate) fn sample_report() -> PresentationReport {
        let to = Utc::now();
        let from = to - Duration::days(7);

        PresentationReport {
            project_name: "Test Project".to_string(),
            window: ReportWindow::new(from, to),
            kinds: vec![ReportKind::Member, ReportKind::Model, ReportKind::Jira],
            totals: ResourceUsageTotals {
                reported_installations: 1,
                installed_installations: 1,
                installed_members: 1,
                pending_installations: 0,
                attention_installations: 0,
                all_requests: 10,
                governed_requests: 5,
                requests: 10,
                resource_uses: 3,
                model_calls: 10,
                tool_calls: 2,
                successes: 9,
                errors: 1,
                blocked: 0,
                cancelled: 0,
                tokens_in: 100_000,
                tokens_out: 20_000,
                cache_read_tokens: 5_000,
                cache_write_tokens: 1_000,
                reasoning_tokens: 0,
                tool_use_tokens: 0,
                total_tokens: 120_000,
                estimated_cost_usd_micros: 12_000_000,
                cache_savings_usd_micros: 500_000,
                unpriced_model_calls: 1,
                average_tokens_per_request: 12_000,
                average_duration_ms: 800,
            },
            daily: vec![
                ResourceUsageDay {
                    date: "2026-09-18".to_string(),
                    requests: 5,
                    successes: 5,
                    errors: 0,
                    blocked: 0,
                    cancelled: 0,
                    tokens_in: 50_000,
                    tokens_out: 10_000,
                    cache_read_tokens: 2_000,
                    reasoning_tokens: 0,
                    tool_use_tokens: 0,
                    estimated_cost_usd_micros: 6_000_000,
                    unpriced_model_calls: 0,
                },
                ResourceUsageDay {
                    date: "2026-09-19".to_string(),
                    requests: 5,
                    successes: 4,
                    errors: 1,
                    blocked: 0,
                    cancelled: 0,
                    tokens_in: 50_000,
                    tokens_out: 10_000,
                    cache_read_tokens: 3_000,
                    reasoning_tokens: 0,
                    tool_use_tokens: 0,
                    estimated_cost_usd_micros: 6_000_000,
                    unpriced_model_calls: 1,
                },
            ],
            members: vec![MemberCostReportRow {
                user_id: Uuid::new_v4(),
                display_name: "Alice".to_string(),
                email: "alice@example.test".to_string(),
                primary_role: PrimaryRole::Admin,
                calls: 10,
                unpriced_calls: 1,
                input_tokens: 100_000,
                output_tokens: 20_000,
                cache_read_tokens: 5_000,
                cache_write_tokens: 1_000,
                reasoning_tokens: 0,
                total_tokens: 120_000,
                total_cost_usd_micros: 12_000_000,
                cache_savings_usd_micros: 500_000,
                avg_usd_micros_per_million_tokens: 100_000,
            }],
            member_totals: ModelCostReportTotals {
                calls: 10,
                unpriced_calls: 1,
                input_tokens: 100_000,
                output_tokens: 20_000,
                cache_read_tokens: 5_000,
                cache_write_tokens: 1_000,
                reasoning_tokens: 0,
                total_tokens: 120_000,
                total_cost_usd_micros: 12_000_000,
                cache_savings_usd_micros: 500_000,
            },
            models: vec![ModelCostReportRow {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-5".to_string(),
                calls: 10,
                unpriced_calls: 1,
                input_tokens: 100_000,
                output_tokens: 20_000,
                cache_read_tokens: 5_000,
                cache_write_tokens: 1_000,
                reasoning_tokens: 0,
                total_tokens: 120_000,
                input_cost_usd_micros: 8_000_000,
                output_cost_usd_micros: 3_500_000,
                cache_read_cost_usd_micros: 400_000,
                cache_write_cost_usd_micros: 100_000,
                total_cost_usd_micros: 12_000_000,
                cache_savings_usd_micros: 500_000,
                avg_usd_micros_per_million_tokens: 100_000,
            }],
            model_totals: ModelCostReportTotals {
                calls: 10,
                unpriced_calls: 1,
                input_tokens: 100_000,
                output_tokens: 20_000,
                cache_read_tokens: 5_000,
                cache_write_tokens: 1_000,
                reasoning_tokens: 0,
                total_tokens: 120_000,
                total_cost_usd_micros: 12_000_000,
                cache_savings_usd_micros: 500_000,
            },
            jira_tasks: vec![TaskCostRow {
                issue_key: "SCRUM-1".to_string(),
                title: "Checkout flow".to_string(),
                resolved_type: "task".to_string(),
                resolved_project: None,
                status: "In Progress".to_string(),
                parent_key: None,
                assignee_display_name: Some("Alice".to_string()),
                matched_user_id: None,
                matched: true,
                precise: true,
                calls: 10,
                total_tokens: 120_000,
                total_cost_usd_micros: 12_000_000,
                cache_savings_usd_micros: 500_000,
                tokens_in: 100_000,
                tokens_out: 20_000,
                cache_read_tokens: 5_000,
                cache_write_tokens: 1_000,
                reasoning_tokens: 0,
                total_duration_ms: 5_000,
                models: vec!["anthropic:claude-sonnet-5".to_string()],
                by_status: Vec::new(),
                rollup_by_status: Vec::new(),
            }],
            callouts: ReportCallouts {
                top_members: vec![TopEntry {
                    label: "Alice".to_string(),
                    total_cost_usd_micros: 12_000_000,
                    total_tokens: 120_000,
                }],
                top_models: vec![TopEntry {
                    label: "anthropic:claude-sonnet-5".to_string(),
                    total_cost_usd_micros: 12_000_000,
                    total_tokens: 120_000,
                }],
                top_tasks: vec![TopEntry {
                    label: "SCRUM-1 — Checkout flow".to_string(),
                    total_cost_usd_micros: 12_000_000,
                    total_tokens: 120_000,
                }],
                cost_change_pct: Some(20.0),
                cache_savings_usd_micros: 500_000,
                cache_savings_change_pct: Some(10.0),
                unpriced_ratio_bps: 1000,
                cost_outlier_tasks: vec![OutlierTask {
                    issue_key: "SCRUM-1".to_string(),
                    title: "Checkout flow".to_string(),
                    resolved_type: "task".to_string(),
                    total_cost_usd_micros: 12_000_000,
                    times_the_type_average: 3.2,
                }],
            },
        }
    }
}
