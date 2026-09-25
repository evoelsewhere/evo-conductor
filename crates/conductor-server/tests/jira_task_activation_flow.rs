//! End-to-end proof that the jira-task-assistant plugin's own activation
//! call is what turns a task from an unmatched/estimated row in Conductor's
//! Jira report into a precisely tracked one -- exercising the exact HTTP
//! call the plugin's `ConductorClient.start_task` makes
//! (`POST /api/v1/client/jira-task-activation`, bearer connection token),
//! then the exact route the console's Jira report page calls
//! (`GET /api/analytics/task-cost-report`), with nothing mocked in between.

mod support;

use axum::http::StatusCode;
use chrono::{Duration, SecondsFormat, Utc};
use conductor_auth::hash_token;
use conductor_domain::PrimaryRole;
use serde_json::json;
use support::test_app;
use uuid::Uuid;

const RAW_TOKEN: &str = "evc_test_jira_activation_flow_canary_token";
const ISSUE_KEY: &str = "SCRUM-9001";

#[tokio::test]
async fn plugin_activation_makes_the_task_cost_report_track_real_usage() {
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for_role(PrimaryRole::Admin).await;

    // The member's Jira account email -- what the report matches a task's
    // assignee against. Set directly, the way the self-service settings
    // route would, since this fixture has no browser session to click
    // through it.
    sqlx::query("UPDATE users SET jira_account_email = ? WHERE id = ?")
        .bind(&member.email)
        .bind(member.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("set jira account email");

    // A synced Jira task assigned to this member.
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO jira_tasks (
            issue_key, project_id, title, issue_type, resolved_type,
            assignee_display_name, assignee_account_id, parent_key, status,
            jira_updated_at, synced_at, assignee_email, resolved_project
        ) VALUES (?, ?, 'Activation flow test task', 'Task', 'task',
                  ?, 'acc-1', NULL, 'In Progress', ?, ?, ?, NULL)
        "#,
    )
    .bind(ISSUE_KEY)
    .bind(project_id.to_string())
    .bind(&member.display_name)
    .bind(&now)
    .bind(&now)
    .bind(&member.email)
    .execute(app.state.db.pool())
    .await
    .expect("seed jira task");

    // The plugin's own connection token (`report_telemetry` scope) -- what a
    // member self-issues from their member page for jira-task-assistant to
    // use. Seeded directly here the same way `connection_secret_hash.rs`
    // does, for the same reason: no browser session in this fixture.
    sqlx::query(
        r#"
        INSERT INTO connection_secrets (
            id, name, prefix, token_hash, owner_user_id, scopes, created_at
        ) VALUES (?, 'jira-task-assistant test', 'evc_test', ?, ?, ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(hash_token(RAW_TOKEN))
    .bind(member.id.to_string())
    .bind(serde_json::to_string(&["report_telemetry"]).expect("serialize scope"))
    .bind(Utc::now().to_rfc3339())
    .execute(app.state.db.pool())
    .await
    .expect("seed connection secret");

    // 1. Exactly what `jira_assistant.conductor_client.ConductorClient.start_task`
    //    sends when the agent calls `jira_start_task` / `jira_detect_task_from_branch`.
    let (status, body) = app
        .post(
            "/api/v1/client/jira-task-activation",
            Some(RAW_TOKEN),
            json!({ "issue_key": ISSUE_KEY }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["recorded"], true);

    let activation_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM task_activations
         WHERE project_id = ? AND user_id = ? AND issue_key = ?",
    )
    .bind(project_id.to_string())
    .bind(member.id.to_string())
    .bind(ISSUE_KEY)
    .fetch_one(app.state.db.pool())
    .await
    .expect("read task_activations");
    assert_eq!(
        activation_count, 1,
        "the plugin's activation call must be recorded"
    );

    // 2. Real usage the member generates *after* starting the task -- the
    //    same telemetry evoflux core already reports on every model call.
    let after_activation = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, project_id, user_id, request_id, event_type, sequence,
            provider, model, tokens_in, tokens_out, cache_read_tokens,
            cache_write_tokens, reasoning_tokens, tool_use_tokens, duration_ms,
            status, server_cost_usd_micros, primary_role_snapshot,
            reported_at, received_at
        ) VALUES (?, ?, ?, ?, 'model_call', 1, 'anthropic', 'claude-sonnet-5',
                  1000, 200, 50, 10, 0, 0, 1200, 'success', 15000, 'user', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(member.id.to_string())
    .bind(Uuid::new_v4().to_string())
    .bind(&after_activation)
    .bind(&after_activation)
    .execute(app.state.db.pool())
    .await
    .expect("seed post-activation telemetry");

    // 3. The console's own Jira report, exactly as an admin loads it -- must
    //    now show this task as precisely tracked, not zero, not an estimate.
    let from = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let to = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let (status, report) = app
        .get(
            &format!("/api/analytics/task-cost-report?from={from}&to={to}"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");

    let rows = report["rows"].as_array().expect("rows");
    let row = rows
        .iter()
        .find(|row| row["issue_key"] == ISSUE_KEY)
        .unwrap_or_else(|| panic!("no row for {ISSUE_KEY} in {report}"));

    assert_eq!(row["matched"], true, "{row}");
    assert_eq!(row["matched_user_id"], member.id.to_string(), "{row}");
    assert_eq!(
        row["precise"], true,
        "the plugin's activation plus real usage after it must make this \
         task precise, not the zero/fallback a merely-matched task shows: {row}"
    );
    assert_eq!(row["calls"], 1, "{row}");
    assert_eq!(row["tokens_in"], 1000, "{row}");
    assert_eq!(row["tokens_out"], 200, "{row}");
    assert_eq!(row["cache_read_tokens"], 50, "{row}");
    assert_eq!(row["cache_write_tokens"], 10, "{row}");
    assert_eq!(row["total_tokens"], 1260, "{row}");
    assert_eq!(row["total_cost_usd_micros"], 15000, "{row}");

    assert_eq!(report["matched_members_total_calls"], 1, "{report}");
    assert_eq!(report["matched_members_total_tokens"], 1260, "{report}");
}

#[tokio::test]
async fn a_matched_task_with_no_activation_call_stays_at_zero_not_an_estimate() {
    // The precise-vs-fallback distinction this whole design exists for: a
    // task assigned (by email) to a real member who has simply never called
    // `jira_start_task` for it must show zero, never that member's whole
    // unrelated period total.
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for_role(PrimaryRole::Admin).await;

    sqlx::query("UPDATE users SET jira_account_email = ? WHERE id = ?")
        .bind(&member.email)
        .bind(member.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("set jira account email");

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO jira_tasks (
            issue_key, project_id, title, issue_type, resolved_type,
            assignee_display_name, assignee_account_id, parent_key, status,
            jira_updated_at, synced_at, assignee_email, resolved_project
        ) VALUES ('SCRUM-9002', ?, 'Never activated test task', 'Task', 'task',
                  ?, 'acc-2', NULL, 'To Do', ?, ?, ?, NULL)
        "#,
    )
    .bind(project_id.to_string())
    .bind(&member.display_name)
    .bind(&now)
    .bind(&now)
    .bind(&member.email)
    .execute(app.state.db.pool())
    .await
    .expect("seed jira task");

    // The member has unrelated usage in this window, but never called
    // `jira_start_task` for this issue.
    sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, project_id, user_id, request_id, event_type, sequence,
            provider, model, tokens_in, tokens_out, duration_ms,
            status, server_cost_usd_micros, primary_role_snapshot,
            reported_at, received_at
        ) VALUES (?, ?, ?, ?, 'model_call', 1, 'anthropic', 'claude-sonnet-5',
                  50000, 9000, 1200, 'success', 900000, 'user', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(member.id.to_string())
    .bind(Uuid::new_v4().to_string())
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed unrelated telemetry");

    let from = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let to = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let (status, report) = app
        .get(
            &format!("/api/analytics/task-cost-report?from={from}&to={to}"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");

    let rows = report["rows"].as_array().expect("rows");
    let row = rows
        .iter()
        .find(|row| row["issue_key"] == "SCRUM-9002")
        .unwrap_or_else(|| panic!("no row for SCRUM-9002 in {report}"));

    assert_eq!(row["matched"], true, "{row}");
    assert_eq!(row["precise"], false, "{row}");
    assert_eq!(row["calls"], 0, "{row}");
    assert_eq!(row["total_tokens"], 0, "{row}");
    assert_eq!(
        row["total_cost_usd_micros"], 0,
        "must never show the member's unrelated whole-period spend: {row}"
    );
}
