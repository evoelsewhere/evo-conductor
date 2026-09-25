//! A local mirror of one Jira project's issues, and the task-level usage
//! report built from it — the follow-on to Phase 4's single fixed-issue
//! comment, letting spend be checked against the person actually assigned
//! to a task rather than only against the project as a whole.
//!
//! Attribution has two layers, tried in order:
//! 1. **Precise, per-task**: the `jira-task-assistant` plugin calls a
//!    connection-authenticated endpoint (`TaskActivationRequest`) whenever
//!    the agent starts work on a task — "this member began task X at T".
//!    The report turns a member's sequence of these into time windows and
//!    sums their telemetry inside each one. This needs no evoflux core
//!    change at all: the plugin calls Conductor directly with its own
//!    connection token, the same way it already calls Jira directly.
//! 2. **Fallback, per-person**: for a member with no activations recorded
//!    (hasn't adopted the plugin flow yet, or Jira's own assignee email
//!    matched but never called the tool), their whole period total is
//!    shown against every task assigned to them — approximate, and the
//!    report says so via `precise: false`.
//!
//! When a task is precise, its usage is further split by the Jira workflow
//! status active at each event's time (`TaskCostRow::by_status`), using
//! `JiraStatusChange` history synced only for issues that have activations.
//! A parent task's `rollup_by_status` folds its subtasks' splits in too, so
//! "how many tokens went into rework after this went back to In Progress"
//! is answerable at both the single-task and parent-task level.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One synced Jira issue. `resolved_type` is recomputed from
/// `JiraSettings.task_type_rules` on every sync — derived, not a second
/// source of truth for the raw `issue_type` Jira itself reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraTask {
    pub issue_key: String,
    pub title: String,
    pub issue_type: String,
    pub resolved_type: String,
    /// From `JiraSettings.project_prefix_rules` — a title-prefix-derived
    /// sub-project/module label (e.g. a `[PST]` tag), for a Jira project
    /// whose issues actually span more than one product. `None` when no
    /// rule matches; there is no Jira field to fall back to.
    pub resolved_project: Option<String>,
    pub assignee_display_name: Option<String>,
    /// Absent when Jira's user-privacy setting hides it from the API
    /// token's account — the report's own matching falls back to showing
    /// the task unmatched, never errors over it.
    pub assignee_email: Option<String>,
    pub assignee_account_id: Option<String>,
    /// Jira's own `parent`/epic-link field — a subtask's parent task, or an
    /// issue's epic. No separate Conductor-side hierarchy.
    pub parent_key: Option<String>,
    pub status: String,
    pub jira_updated_at: Option<DateTime<Utc>>,
    pub synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraTaskListResponse {
    pub tasks: Vec<JiraTask>,
}

/// One synced task, with the assignee's own usage for the report window
/// attached when the match succeeds. See the module docs for what
/// `total_cost_usd_micros` etc. actually mean here — a person's total, not
/// a task-specific slice.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskCostRow {
    pub issue_key: String,
    pub title: String,
    pub resolved_type: String,
    pub resolved_project: Option<String>,
    pub status: String,
    pub parent_key: Option<String>,
    pub assignee_display_name: Option<String>,
    /// The matched member's own id, so the UI can deep-link a request row
    /// straight into that member's own request-detail page -- the same
    /// page a member sees for their own activity, reused here rather than
    /// building a second one. `None` exactly when `matched` is `false`.
    pub matched_user_id: Option<Uuid>,
    /// `false` when the assignee has no email, or no member's
    /// `jira_account_email` matches it — the row still renders, with all
    /// usage figures at zero, so an unmatched task stays visible rather
    /// than silently dropping out of the list.
    pub matched: bool,
    /// `true` when `calls`/`total_tokens`/`total_cost_usd_micros` come from
    /// the assignee's own recorded task-activation windows (a real per-task
    /// figure); `false` when they're that member's whole period total
    /// (approximate fallback) or the task is unmatched (always zero).
    pub precise: bool,
    pub calls: u64,
    pub total_tokens: u64,
    pub total_cost_usd_micros: u64,
    /// How much cheaper this task's cache reads were than paying full input
    /// price for the same tokens; zero for a fallback row, same as the token
    /// split below.
    pub cache_savings_usd_micros: u64,
    /// The four figures below split `total_tokens` -- only meaningful when
    /// `precise` is true; a fallback row leaves them all zero rather than
    /// implying a split of a whole-person total it doesn't actually have.
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    /// Summed across every event counted toward this task; zero for a
    /// fallback row for the same reason as the token split above.
    pub total_duration_ms: u64,
    /// Distinct `provider:model` strings seen across this task's counted
    /// events, in first-seen order.
    pub models: Vec<String>,
    /// This task's own usage split by the Jira workflow status active at
    /// each event's time -- only populated when `precise` is true; a
    /// PIC-total fallback row has no per-event timestamps to split by.
    /// Empty, not a zero row, when there's nothing to show.
    pub by_status: Vec<StatusTokenBreakdown>,
    /// `by_status` plus every descendant's `by_status`, merged by status --
    /// equal to `by_status` for a task with no children. Lets a parent
    /// task's card show "this task alone" vs. "this task and its subtasks"
    /// without the UI re-deriving the tree sum itself.
    pub rollup_by_status: Vec<StatusTokenBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct StatusTokenBreakdown {
    pub status: String,
    pub calls: u64,
    pub total_tokens: u64,
    pub total_cost_usd_micros: u64,
}

/// One request counted toward a precise task's numbers -- the drill-down
/// behind a "Tracked" task's summary, in the same spirit as a member's own
/// activity list but scoped to this one issue's activation windows. Only
/// ever populated for a precise task; a fallback (whole-person) task has no
/// per-request slice to list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskActivityItem {
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_name: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub duration_ms: u64,
    pub total_cost_usd_micros: u64,
    pub cache_savings_usd_micros: u64,
    pub status: String,
    /// The Jira workflow status active at the moment of this request.
    pub jira_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskActivityResponse {
    pub issue_key: String,
    pub items: Vec<TaskActivityItem>,
}

/// Body for the connection-authenticated endpoint the jira-task-assistant
/// plugin calls directly (its own connection token, no evoflux involvement)
/// to record "the calling member started this task now".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskActivationRequest {
    pub issue_key: String,
}

/// One synced status transition for an issue, from Jira's own changelog --
/// "this issue moved to `status` at `changed_at`". Stored so the report
/// builder can tell which status was active when a given telemetry event
/// happened, without re-fetching Jira per report request. Only synced for
/// issues that actually have a `task_activations` row -- most Jira issues in
/// a project are never opened through the plugin, so there's no per-task
/// usage to split by status for them anyway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraStatusChange {
    pub issue_key: String,
    pub status: String,
    pub changed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCostReport {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub rows: Vec<TaskCostRow>,
    pub matched_tasks: u64,
    pub unmatched_tasks: u64,
    /// Summed across `precise` rows only -- real, tracked work. Never a
    /// person's whole-period total: a matched-but-not-yet-tracked task
    /// contributes zero here, the same zero it shows in its own row.
    pub matched_members_total_calls: u64,
    pub matched_members_total_tokens: u64,
    pub matched_members_total_cost_usd_micros: u64,
}
