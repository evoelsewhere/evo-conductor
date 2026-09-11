//! The console's face on where project spend actually goes, broken down by
//! model and by the token components a rate prices separately.
//!
//! What matters here is that the wiring from stored telemetry through the
//! rate table to the HTTP response holds together end to end -- the split
//! and scaling math already has unit coverage in `model_cost_report.rs`.

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
        ) VALUES (?, 'Model cost report test', '127.0.0.1', 4700, 'L1', 1, 'unused', ?, ?)
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
    cache_read_tokens: i64,
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
        ) VALUES (?, ?, ?, ?, 'model_call', 1, ?, ?, ?, ?, ?, 0, 0, 0, 10,
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
    .bind(cache_read_tokens)
    .bind(cost_micros)
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed call");
}

#[tokio::test]
async fn an_admin_reads_a_report_split_by_model_and_component() {
    let app = test_app().await;
    let project_id = seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;
    let member = app.seed_user(PrimaryRole::User).await;

    conductor_server::core::model_pricing::apply_catalog(
        &app.state.db,
        br#"{"anthropic":{"id":"anthropic","models":{"claude-sonnet-4":{
            "id":"claude-sonnet-4","cost":{"input":3,"output":15}}}}}"#,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");
    app.state
        .model_rates
        .reload(&app.state.db)
        .await
        .expect("reload rate table");
    // A model with a current rate: cache-read tokens fold into input since
    // this catalog carries no cache rate for it, and the stored total is
    // deliberately independent of the rate so the split is a proportion of
    // an authoritative figure, not a re-derivation of it.
    seed_call(
        &app,
        project_id,
        member.id,
        "anthropic",
        "claude-sonnet-4",
        1_000_000,
        200_000,
        100_000,
        Some(12_345_000),
    )
    .await;
    seed_call(
        &app,
        project_id,
        member.id,
        "anthropic",
        "claude-sonnet-4",
        500_000,
        0,
        0,
        None,
    )
    .await;

    // A model Conductor has never priced: its whole spend must surface as
    // unattributed rather than guessed at.
    seed_call(
        &app,
        project_id,
        member.id,
        "mystery",
        "ghost-model",
        10_000,
        10_000,
        0,
        Some(999_999_000),
    )
    .await;

    let (status, report) = app
        .get("/api/analytics/model-cost-report", Some(&admin))
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");

    let rows = report["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 2, "one row per model");

    // Storage orders by cost, and splitting must not disturb that order.
    let ghost = &rows[0];
    assert_eq!(ghost["model"], "ghost-model");
    assert_eq!(ghost["total_cost_usd_micros"], 999_999_000);
    assert_eq!(
        ghost["input_cost_usd_micros"], 0,
        "no current rate means no attributed split"
    );
    assert_eq!(ghost["cache_write_cost_usd_micros"], 0);

    let sonnet = &rows[1];
    assert_eq!(sonnet["model"], "claude-sonnet-4");
    assert_eq!(sonnet["calls"], 2);
    assert_eq!(
        sonnet["unpriced_calls"], 1,
        "the second call has no cost yet"
    );
    assert_eq!(sonnet["input_tokens"], 1_500_000);
    assert_eq!(sonnet["output_tokens"], 200_000);
    assert_eq!(sonnet["cache_read_tokens"], 100_000);
    assert_eq!(sonnet["total_cost_usd_micros"], 12_345_000);
    let component_sum = sonnet["input_cost_usd_micros"].as_u64().unwrap()
        + sonnet["output_cost_usd_micros"].as_u64().unwrap()
        + sonnet["cache_read_cost_usd_micros"].as_u64().unwrap()
        + sonnet["cache_write_cost_usd_micros"].as_u64().unwrap();
    assert_eq!(
        component_sum, 12_345_000,
        "the four components must always add back up to the stored total"
    );
    assert!(
        sonnet["input_cost_usd_micros"].as_u64().unwrap() > 0,
        "cache-read tokens with no cache rate fold into input, so input carries the split"
    );

    let totals = &report["totals"];
    assert_eq!(totals["calls"], 3);
    assert_eq!(totals["unpriced_calls"], 1);
    assert_eq!(
        totals["total_cost_usd_micros"],
        12_345_000u64 + 999_999_000,
        "the report total must match the sum of every row, same as every other cost view"
    );
}

#[tokio::test]
async fn role_and_provider_filters_narrow_the_model_report() {
    let app = test_app().await;
    let project_id = seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let contributor = app.seed_user(PrimaryRole::Contribute).await;

    seed_call(
        &app,
        project_id,
        member.id,
        "anthropic",
        "claude-sonnet-4",
        100_000,
        10_000,
        0,
        Some(1_000_000),
    )
    .await;
    seed_call(
        &app,
        project_id,
        contributor.id,
        "openai",
        "gpt-5",
        200_000,
        20_000,
        0,
        Some(2_000_000),
    )
    .await;

    let (status, by_role) = app
        .get(
            "/api/analytics/model-cost-report?primary_role=contribute",
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{by_role}");
    let rows = by_role["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "only the contributor's model");
    assert_eq!(rows[0]["model"], "gpt-5");

    let (status, by_provider) = app
        .get(
            "/api/analytics/model-cost-report?provider=anthropic",
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{by_provider}");
    let rows = by_provider["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "only anthropic calls");
    assert_eq!(rows[0]["model"], "claude-sonnet-4");
}

#[tokio::test]
async fn a_user_without_telemetry_read_is_denied() {
    let app = test_app().await;
    seed_project(&app).await;
    let member_token = app.token_for_role(PrimaryRole::User).await;

    let (status, _) = app
        .get("/api/analytics/model-cost-report", Some(&member_token))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
