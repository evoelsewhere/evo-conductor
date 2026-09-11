//! The console's face on where project spend goes by member.
//!
//! What matters here is that the wiring from stored telemetry to the HTTP
//! response holds together end to end, and that the role/tag/provider/model
//! filters shared with the model cost report actually narrow the rows --
//! the aggregation math itself mirrors `model_cost_report.rs`.

mod support;

use axum::http::StatusCode;
use chrono::Utc;
use conductor_domain::PrimaryRole;
use support::{test_app, TestApp};
use uuid::Uuid;

async fn seed_project(app: &TestApp) -> Uuid {
    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, bind_host, bind_port, collection_level,
            setup_completed, jwt_secret, created_at, updated_at
        ) VALUES (?, 'Member cost report test', '127.0.0.1', 4700, 'L1', 1, 'unused', ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed project");
    id
}

#[allow(clippy::too_many_arguments)]
async fn seed_call(
    app: &TestApp,
    project_id: Uuid,
    user_id: Uuid,
    provider: &str,
    model: &str,
    tokens_in: i64,
    tokens_out: i64,
    cost_micros: Option<i64>,
) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, project_id, user_id, request_id, event_type, sequence,
            provider, model, tokens_in, tokens_out, cache_read_tokens,
            cache_write_tokens, reasoning_tokens, tool_use_tokens, duration_ms,
            status, server_cost_usd_micros, primary_role_snapshot,
            reported_at, received_at
        ) VALUES (?, ?, ?, ?, 'model_call', 1, ?, ?, ?, ?, 0, 0, 0, 0, 10,
                  'success', ?, 'user', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(user_id.to_string())
    .bind(Uuid::new_v4().to_string())
    .bind(provider)
    .bind(model)
    .bind(tokens_in)
    .bind(tokens_out)
    .bind(cost_micros)
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed call");
}

#[tokio::test]
async fn an_admin_reads_a_report_split_by_member() {
    let app = test_app().await;
    let project_id = seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;
    let alice = app.seed_user(PrimaryRole::User).await;
    let bob = app.seed_user(PrimaryRole::Contribute).await;

    seed_call(
        &app,
        project_id,
        alice.id,
        "anthropic",
        "claude-sonnet-4",
        1_000_000,
        200_000,
        Some(9_000_000),
    )
    .await;
    seed_call(
        &app, project_id, alice.id, "openai", "gpt-5", 100_000, 20_000, None,
    )
    .await;
    seed_call(
        &app,
        project_id,
        bob.id,
        "anthropic",
        "claude-sonnet-4",
        50_000,
        10_000,
        Some(1_000_000),
    )
    .await;

    let (status, report) = app
        .get("/api/analytics/member-cost-report", Some(&admin))
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");

    let rows = report["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 2, "one row per member");

    // Storage orders by cost, highest spend first.
    let top = &rows[0];
    assert_eq!(top["display_name"], alice.display_name);
    assert_eq!(top["primary_role"], "user");
    assert_eq!(top["calls"], 2);
    assert_eq!(top["unpriced_calls"], 1);
    assert_eq!(top["total_cost_usd_micros"], 9_000_000);
    assert_eq!(top["input_tokens"], 1_100_000);

    let second = &rows[1];
    assert_eq!(second["display_name"], bob.display_name);
    assert_eq!(second["primary_role"], "contribute");
    assert_eq!(second["total_cost_usd_micros"], 1_000_000);

    let totals = &report["totals"];
    assert_eq!(totals["calls"], 3);
    assert_eq!(
        totals["total_cost_usd_micros"], 10_000_000,
        "the report total must match the sum of every row"
    );
}

#[tokio::test]
async fn role_and_provider_filters_narrow_the_member_report() {
    let app = test_app().await;
    let project_id = seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;
    let alice = app.seed_user(PrimaryRole::User).await;
    let bob = app.seed_user(PrimaryRole::Contribute).await;

    seed_call(
        &app,
        project_id,
        alice.id,
        "anthropic",
        "claude-sonnet-4",
        100_000,
        10_000,
        Some(1_000_000),
    )
    .await;
    seed_call(
        &app,
        project_id,
        bob.id,
        "openai",
        "gpt-5",
        200_000,
        20_000,
        Some(2_000_000),
    )
    .await;

    let (status, by_role) = app
        .get(
            "/api/analytics/member-cost-report?primary_role=contribute",
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{by_role}");
    let rows = by_role["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "only the contributor's spend");
    assert_eq!(rows[0]["display_name"], bob.display_name);

    let (status, by_provider) = app
        .get(
            "/api/analytics/member-cost-report?provider=anthropic",
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{by_provider}");
    let rows = by_provider["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "only calls against anthropic");
    assert_eq!(rows[0]["display_name"], alice.display_name);
}

#[tokio::test]
async fn a_user_without_telemetry_read_is_denied() {
    let app = test_app().await;
    seed_project(&app).await;
    let member_token = app.token_for_role(PrimaryRole::User).await;

    let (status, _) = app
        .get("/api/analytics/member-cost-report", Some(&member_token))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
