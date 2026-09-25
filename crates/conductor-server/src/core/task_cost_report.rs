//! Task-level usage report: each synced Jira task paired with its usage for
//! the window, matched by email or, when available, sharpened to a real
//! per-task slice by the jira-task-assistant plugin's own activation calls.
//!
//! EvoFlux has no notion of "this session was for task X" — deliberately;
//! adding one would mean touching its core session model and telemetry
//! pipeline for every install. So attribution has two layers, tried in
//! order for each task:
//!
//! 1. **Precise, per-task**: the task's matched member (see below) has at
//!    least one `task_activations` row for this exact issue. The plugin
//!    calls Conductor directly (its own connection token, no evoflux
//!    involvement) whenever the agent starts work on an issue; a member's
//!    sequence of these becomes contiguous time windows, and this task's
//!    number is just that member's telemetry inside the windows that name
//!    it — a real slice, not an estimate. Inside a precise task, usage is
//!    further split by the Jira workflow status active at each event's
//!    time (`by_status`), using `JiraStatusChange` history synced only for
//!    activated issues.
//! 2. **Not tracked yet**: matched to a real member, but no activation
//!    recorded for this exact issue. This shows zero, not that member's
//!    whole-period total — a task nobody has recorded working on has no
//!    honest number to show, and showing one anyway (an earlier version of
//!    this report did) reads as "why is this charging me before I even
//!    started" the moment someone actually looks. `precise: false` and an
//!    empty `by_status` mark the row this way.
//!
//! `TaskCostReport::matched_members_total_*` is the sum across every
//! *precise* row only — real, tracked work, never a person's unrelated
//! whole-period activity. It can be smaller than a member's true total
//! spend; that's intentional; it exists to answer "how much tracked work
//! happened", not "how much did this person spend".
//!
//! A parent task's `rollup_by_status` folds its subtasks' splits in too
//! (recursively), so "how much of this went into rework after this went
//! back to In Progress" is answerable at both the single-task and the
//! whole-parent level.
//!
//! Matching a task to a member tries two things, in order:
//! 1. `assignee_email` (Jira) == `jira_account_email` (member), exact.
//! 2. Jira hides email from the API for most accounts by default (a
//!    per-user Atlassian privacy setting, not something Conductor
//!    controls) — `assignee_email` then syncs as empty. A newly created
//!    Atlassian account with no display name set shows the email's own
//!    local part (before `@`) as its `assignee_display_name`, so that is
//!    tried as a fallback: the member's configured email's local part
//!    against the task's `assignee_display_name`, case-insensitive. This
//!    only ever narrows a match that email alone missed — it never
//!    overrides an actual email mismatch.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use conductor_domain::{
    ConductorError, JiraStatusChange, StatusTokenBreakdown, TaskActivityItem, TaskActivityResponse,
    TaskCostReport, TaskCostRow,
};
use conductor_storage::repos::{RawTelemetryEventSlice, TaskActivation};
use conductor_storage::Db;
use uuid::Uuid;

use crate::core::error::ApiResult;

/// `[start, end)` of one activation, plus which issue it names — built from
/// a member's own consecutive `task_activations` rows.
struct Window {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    issue_key: String,
}

fn build_windows(activations: &[TaskActivation], report_to: DateTime<Utc>) -> Vec<Window> {
    activations
        .iter()
        .enumerate()
        .map(|(i, activation)| Window {
            start: activation.started_at,
            end: activations
                .get(i + 1)
                .map(|next| next.started_at)
                .unwrap_or(report_to),
            issue_key: activation.issue_key.clone(),
        })
        .collect()
}

/// The Jira workflow status active at `at`, from one issue's own history
/// (oldest first). Falls back to the task's currently-synced status when
/// history doesn't cover `at` at all -- either no changelog was ever synced
/// for this issue, or `at` predates even the synthetic "before the first
/// known transition" entry (which only exists when Jira's changelog itself
/// recorded a `fromString` for the earliest transition).
fn status_at<'a>(history: &'a [JiraStatusChange], at: DateTime<Utc>, fallback: &'a str) -> &'a str {
    history
        .iter()
        .rev()
        .find(|change| change.changed_at <= at)
        .map(|change| change.status.as_str())
        .unwrap_or(fallback)
}

#[derive(Default, Clone, Copy)]
struct Accumulator {
    calls: u64,
    total_tokens: u64,
    total_cost_usd_micros: u64,
}

impl Accumulator {
    fn add(&mut self, event: &RawTelemetryEventSlice) {
        self.calls += 1;
        self.total_tokens += event.total_tokens;
        self.total_cost_usd_micros += event.total_cost_usd_micros;
    }
}

fn breakdown_from_map(map: HashMap<String, Accumulator>) -> Vec<StatusTokenBreakdown> {
    let mut out: Vec<StatusTokenBreakdown> = map
        .into_iter()
        .map(|(status, acc)| StatusTokenBreakdown {
            status,
            calls: acc.calls,
            total_tokens: acc.total_tokens,
            total_cost_usd_micros: acc.total_cost_usd_micros,
        })
        .collect();
    out.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    out
}

fn merge_breakdowns(
    a: &[StatusTokenBreakdown],
    b: &[StatusTokenBreakdown],
) -> Vec<StatusTokenBreakdown> {
    let mut map: HashMap<String, Accumulator> = HashMap::new();
    for item in a.iter().chain(b.iter()) {
        let entry = map.entry(item.status.clone()).or_default();
        entry.calls += item.calls;
        entry.total_tokens += item.total_tokens;
        entry.total_cost_usd_micros += item.total_cost_usd_micros;
    }
    breakdown_from_map(map)
}

/// A precise task's own totals, split by token kind and by model, plus the
/// raw events themselves -- the latter only ever read by the activity
/// drill-down, never by the summary report.
#[derive(Default)]
struct TaskAccumulator {
    calls: u64,
    tokens_in: u64,
    tokens_out: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
    total_tokens: u64,
    total_duration_ms: u64,
    total_cost_usd_micros: u64,
    cache_savings_usd_micros: u64,
    models: Vec<String>,
    models_seen: HashSet<String>,
}

impl TaskAccumulator {
    fn add(&mut self, event: &RawTelemetryEventSlice) {
        self.calls += 1;
        self.tokens_in += event.tokens_in;
        self.tokens_out += event.tokens_out;
        self.cache_read_tokens += event.cache_read_tokens;
        self.cache_write_tokens += event.cache_write_tokens;
        self.reasoning_tokens += event.reasoning_tokens;
        self.total_tokens += event.total_tokens;
        self.total_duration_ms += event.duration_ms;
        self.total_cost_usd_micros += event.total_cost_usd_micros;
        self.cache_savings_usd_micros += event.cache_savings_usd_micros;
        if let (Some(provider), Some(model)) = (&event.provider, &event.model) {
            let key = format!("{provider}:{model}");
            if self.models_seen.insert(key.clone()) {
                self.models.push(key);
            }
        }
    }
}

/// One matched member's precise, per-issue slice of their own telemetry --
/// only ever built for members who have at least one activation recorded,
/// and only ever used for the exact issues that name them.
struct PreciseUserData {
    per_issue: HashMap<String, TaskAccumulator>,
    per_issue_by_status: HashMap<String, HashMap<String, Accumulator>>,
}

/// Assigns each event to the window covering its time, skipping anything
/// before the member's very first recorded activation -- usage from before
/// they adopted the plugin flow, not attributable to any specific task.
/// Shared by the summary computation and the activity drill-down so the two
/// can never disagree about which events belong to which task.
fn assign_events_to_windows<'a>(
    windows: &'a [Window],
    events: &'a [RawTelemetryEventSlice],
) -> Vec<(&'a Window, &'a RawTelemetryEventSlice)> {
    let mut out = Vec::new();
    let mut window_idx = 0usize;
    for event in events {
        while window_idx < windows.len() && event.received_at >= windows[window_idx].end {
            window_idx += 1;
        }
        let Some(window) = windows.get(window_idx) else {
            break;
        };
        if event.received_at < window.start {
            continue;
        }
        out.push((window, event));
    }
    out
}

fn compute_precise_for_user(
    activations: &[TaskActivation],
    events: &[RawTelemetryEventSlice],
    report_to: DateTime<Utc>,
    status_history_by_issue: &HashMap<String, Vec<JiraStatusChange>>,
    task_status_by_key: &HashMap<String, String>,
) -> PreciseUserData {
    let windows = build_windows(activations, report_to);
    let mut per_issue: HashMap<String, TaskAccumulator> = HashMap::new();
    let mut per_issue_by_status: HashMap<String, HashMap<String, Accumulator>> = HashMap::new();

    for (window, event) in assign_events_to_windows(&windows, events) {
        per_issue.entry(window.issue_key.clone()).or_default().add(event);

        let empty = Vec::new();
        let history = status_history_by_issue
            .get(&window.issue_key)
            .unwrap_or(&empty);
        let fallback = task_status_by_key
            .get(&window.issue_key)
            .map(String::as_str)
            .unwrap_or("");
        let status = status_at(history, event.received_at, fallback).to_string();
        per_issue_by_status
            .entry(window.issue_key.clone())
            .or_default()
            .entry(status)
            .or_default()
            .add(event);
    }

    PreciseUserData {
        per_issue,
        per_issue_by_status,
    }
}

/// `rows[i].by_status` plus every descendant's, merged by status. A fresh
/// visited-set per row rather than one shared across the whole pass: each
/// row's rollup is its own independent traversal, only guarded against a
/// cycle within itself.
fn compute_rollups(rows: &[TaskCostRow]) -> Vec<Vec<StatusTokenBreakdown>> {
    let mut children: HashMap<&str, Vec<usize>> = HashMap::new();
    let index_by_key: HashMap<&str, usize> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| (row.issue_key.as_str(), i))
        .collect();
    for (i, row) in rows.iter().enumerate() {
        if let Some(parent) = row.parent_key.as_deref() {
            if index_by_key.contains_key(parent) {
                children.entry(parent).or_default().push(i);
            }
        }
    }

    fn collect(
        i: usize,
        rows: &[TaskCostRow],
        children: &HashMap<&str, Vec<usize>>,
        visited: &mut [bool],
    ) -> Vec<StatusTokenBreakdown> {
        if visited[i] {
            return Vec::new();
        }
        visited[i] = true;
        let mut merged = rows[i].by_status.clone();
        if let Some(kids) = children.get(rows[i].issue_key.as_str()) {
            for &kid in kids {
                let child_rollup = collect(kid, rows, children, visited);
                merged = merge_breakdowns(&merged, &child_rollup);
            }
        }
        merged
    }

    (0..rows.len())
        .map(|i| {
            let mut visited = vec![false; rows.len()];
            collect(i, rows, &children, &mut visited)
        })
        .collect()
}

/// Case-insensitive: Jira and a member's own typed-in email rarely agree on
/// casing, and an exact-match requirement here would just make every task
/// read as unmatched for no real reason.
fn build_email_maps(
    members: &[conductor_domain::User],
) -> (HashMap<String, Uuid>, HashMap<String, Uuid>) {
    let mut email_to_user_id: HashMap<String, Uuid> = HashMap::new();
    let mut local_part_to_user_id: HashMap<String, Uuid> = HashMap::new();
    for member in members {
        let Some(email) = member.jira_account_email.as_deref() else {
            continue;
        };
        let email = email.trim().to_lowercase();
        if email.is_empty() {
            continue;
        }
        if let Some((local_part, _domain)) = email.split_once('@') {
            local_part_to_user_id
                .entry(local_part.to_string())
                .or_insert(member.id);
        }
        email_to_user_id.entry(email).or_insert(member.id);
    }
    (email_to_user_id, local_part_to_user_id)
}

fn matched_user_id_for_task(
    task: &conductor_domain::JiraTask,
    email_to_user_id: &HashMap<String, Uuid>,
    local_part_to_user_id: &HashMap<String, Uuid>,
) -> Option<Uuid> {
    task.assignee_email
        .as_deref()
        .map(|email| email.trim().to_lowercase())
        .filter(|email| !email.is_empty())
        .and_then(|email| email_to_user_id.get(&email).copied())
        .or_else(|| {
            let display_name = task.assignee_display_name.as_deref()?.trim().to_lowercase();
            if display_name.is_empty() {
                return None;
            }
            local_part_to_user_id.get(&display_name).copied()
        })
}

// The numeric part of a row, uniform across all attribution cases below --
// only how it's filled in differs.
struct Numbers {
    calls: u64,
    tokens_in: u64,
    tokens_out: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
    total_tokens: u64,
    total_duration_ms: u64,
    total_cost_usd_micros: u64,
    cache_savings_usd_micros: u64,
    models: Vec<String>,
    by_status: Vec<StatusTokenBreakdown>,
}

impl Default for Numbers {
    fn default() -> Self {
        Numbers {
            calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            total_tokens: 0,
            total_duration_ms: 0,
            total_cost_usd_micros: 0,
            cache_savings_usd_micros: 0,
            models: Vec::new(),
            by_status: Vec::new(),
        }
    }
}

pub async fn build(
    db: &Db,
    project_id: Uuid,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> ApiResult<TaskCostReport> {
    let tasks = db.jira_tasks().list(project_id).await?;
    let members = db.users().list().await?;

    let (email_to_user_id, local_part_to_user_id) = build_email_maps(&members);

    let matched_user_id_of_task: Vec<Option<Uuid>> = tasks
        .iter()
        .map(|task| matched_user_id_for_task(task, &email_to_user_id, &local_part_to_user_id))
        .collect();

    let task_status_by_key: HashMap<String, String> = tasks
        .iter()
        .map(|task| (task.issue_key.clone(), task.status.clone()))
        .collect();

    let mut status_history_by_issue: HashMap<String, Vec<JiraStatusChange>> = HashMap::new();
    for change in db.jira_status_history().list_for_project(project_id).await? {
        status_history_by_issue
            .entry(change.issue_key.clone())
            .or_default()
            .push(change);
    }

    // Precise data is only ever computed for members this report actually
    // matched to a task -- a member with no matched task has nothing for it
    // to sharpen.
    let mut precise_by_user: HashMap<Uuid, PreciseUserData> = HashMap::new();
    for user_id in matched_user_id_of_task
        .iter()
        .flatten()
        .collect::<HashSet<_>>()
    {
        let activations = db
            .task_activations()
            .list_for_user(project_id, *user_id)
            .await?;
        if activations.is_empty() {
            continue;
        }
        let events = db
            .telemetry()
            .member_event_series(project_id, *user_id, from, to)
            .await?;
        precise_by_user.insert(
            *user_id,
            compute_precise_for_user(
                &activations,
                &events,
                to,
                &status_history_by_issue,
                &task_status_by_key,
            ),
        );
    }

    let mut matched_tasks = 0u64;
    let mut unmatched_tasks = 0u64;
    let mut matched_members_total_calls = 0u64;
    let mut matched_members_total_tokens = 0u64;
    let mut matched_members_total_cost_usd_micros = 0u64;

    let mut rows: Vec<TaskCostRow> = Vec::with_capacity(tasks.len());
    for (task, matched_user_id) in tasks.into_iter().zip(matched_user_id_of_task) {
        let precise_entry = matched_user_id.and_then(|user_id| {
            let precise = precise_by_user.get(&user_id)?;
            let totals = precise.per_issue.get(&task.issue_key)?;
            let by_status = precise
                .per_issue_by_status
                .get(&task.issue_key)
                .cloned()
                .map(breakdown_from_map)
                .unwrap_or_default();
            Some((totals, by_status))
        });

        let (matched, precise, numbers) = match (matched_user_id, precise_entry) {
            (Some(_), Some((precise_totals, by_status))) => {
                matched_tasks += 1;
                matched_members_total_calls += precise_totals.calls;
                matched_members_total_tokens += precise_totals.total_tokens;
                matched_members_total_cost_usd_micros += precise_totals.total_cost_usd_micros;
                (
                    true,
                    true,
                    Numbers {
                        calls: precise_totals.calls,
                        tokens_in: precise_totals.tokens_in,
                        tokens_out: precise_totals.tokens_out,
                        cache_read_tokens: precise_totals.cache_read_tokens,
                        cache_write_tokens: precise_totals.cache_write_tokens,
                        reasoning_tokens: precise_totals.reasoning_tokens,
                        total_tokens: precise_totals.total_tokens,
                        total_duration_ms: precise_totals.total_duration_ms,
                        total_cost_usd_micros: precise_totals.total_cost_usd_micros,
                        cache_savings_usd_micros: precise_totals.cache_savings_usd_micros,
                        models: precise_totals.models.clone(),
                        by_status,
                    },
                )
            }
            // Matched to a member by email, but that member has never
            // called `jira_start_task` for this issue -- there is no
            // honest per-task number to show. Deliberately zero, not that
            // member's whole-period total: showing a dollar figure against
            // a task nobody has recorded working on is exactly the
            // "why is it charging me before I even started" confusion this
            // two-layer design exists to avoid.
            (Some(_), None) => {
                matched_tasks += 1;
                (true, false, Numbers::default())
            }
            (None, _) => {
                unmatched_tasks += 1;
                (false, false, Numbers::default())
            }
        };

        rows.push(TaskCostRow {
            issue_key: task.issue_key,
            title: task.title,
            resolved_type: task.resolved_type,
            resolved_project: task.resolved_project,
            status: task.status,
            parent_key: task.parent_key,
            assignee_display_name: task.assignee_display_name,
            matched_user_id,
            matched,
            precise,
            calls: numbers.calls,
            total_tokens: numbers.total_tokens,
            total_cost_usd_micros: numbers.total_cost_usd_micros,
            cache_savings_usd_micros: numbers.cache_savings_usd_micros,
            tokens_in: numbers.tokens_in,
            tokens_out: numbers.tokens_out,
            cache_read_tokens: numbers.cache_read_tokens,
            cache_write_tokens: numbers.cache_write_tokens,
            reasoning_tokens: numbers.reasoning_tokens,
            total_duration_ms: numbers.total_duration_ms,
            models: numbers.models,
            by_status: numbers.by_status,
            rollup_by_status: Vec::new(),
        });
    }

    let rollups = compute_rollups(&rows);
    for (row, rollup) in rows.iter_mut().zip(rollups) {
        row.rollup_by_status = rollup;
    }

    Ok(TaskCostReport {
        from,
        to,
        rows,
        matched_tasks,
        unmatched_tasks,
        matched_members_total_calls,
        matched_members_total_tokens,
        matched_members_total_cost_usd_micros,
    })
}

/// The request-by-request drill-down behind one precise task's summary --
/// empty (not an error) for a task that's unmatched, or matched but not yet
/// tracked: neither has a per-request slice to show.
pub async fn task_activity_detail(
    db: &Db,
    project_id: Uuid,
    issue_key: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> ApiResult<TaskActivityResponse> {
    let task = db
        .jira_tasks()
        .find(project_id, issue_key)
        .await?
        .ok_or_else(|| ConductorError::NotFound(issue_key.to_string()))?;
    let members = db.users().list().await?;
    let (email_to_user_id, local_part_to_user_id) = build_email_maps(&members);
    let Some(user_id) = matched_user_id_for_task(&task, &email_to_user_id, &local_part_to_user_id)
    else {
        return Ok(TaskActivityResponse {
            issue_key: task.issue_key,
            items: Vec::new(),
        });
    };

    let activations = db
        .task_activations()
        .list_for_user(project_id, user_id)
        .await?;
    if activations.is_empty() {
        return Ok(TaskActivityResponse {
            issue_key: task.issue_key,
            items: Vec::new(),
        });
    }
    let events = db
        .telemetry()
        .member_event_series(project_id, user_id, from, to)
        .await?;
    let windows = build_windows(&activations, to);

    let history = db
        .jira_status_history()
        .list_for_project(project_id)
        .await?
        .into_iter()
        .filter(|change| change.issue_key == issue_key)
        .collect::<Vec<_>>();

    let mut items: Vec<TaskActivityItem> = assign_events_to_windows(&windows, &events)
        .into_iter()
        .filter(|(window, _)| window.issue_key == issue_key)
        .map(|(_, event)| TaskActivityItem {
            request_id: event.request_id.clone(),
            session_id: event.session_id.clone(),
            agent_name: event.agent_name.clone(),
            occurred_at: event.received_at,
            provider: event.provider.clone(),
            model: event.model.clone(),
            tokens_in: event.tokens_in,
            tokens_out: event.tokens_out,
            cache_read_tokens: event.cache_read_tokens,
            cache_write_tokens: event.cache_write_tokens,
            reasoning_tokens: event.reasoning_tokens,
            total_tokens: event.total_tokens,
            duration_ms: event.duration_ms,
            total_cost_usd_micros: event.total_cost_usd_micros,
            cache_savings_usd_micros: event.cache_savings_usd_micros,
            status: event.status.clone(),
            jira_status: status_at(&history, event.received_at, &task.status).to_string(),
        })
        .collect();
    items.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at));

    Ok(TaskActivityResponse {
        issue_key: task.issue_key,
        items,
    })
}
