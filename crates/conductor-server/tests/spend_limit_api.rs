//! The console's face on spend limits and the price catalog.
//!
//! The manifest suite already proves who may call these. What matters here is
//! what they do with what they are given — above all, that an allowance aimed
//! at nobody is refused, because one that matches no telemetry reports zero
//! spend for ever and reads exactly like a subject who has not spent.

mod support;

use axum::http::StatusCode;
use chrono::Utc;
use conductor_domain::PrimaryRole;
use serde_json::json;
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
        ) VALUES (?, 'Spend API test', '127.0.0.1', 4700, 'L1', 1, 'unused', ?, ?)
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

async fn seed_spend(app: &TestApp, project_id: Uuid, user_id: Uuid, cost_micros: i64) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, project_id, user_id, request_id, event_type, sequence,
            provider, model, tokens_in, tokens_out, cache_read_tokens,
            cache_write_tokens, reasoning_tokens, tool_use_tokens, duration_ms,
            status, server_cost_usd_micros, primary_role_snapshot,
            reported_at, received_at
        ) VALUES (?, ?, ?, 'req', 'model_call', 1, 'anthropic', 'claude-sonnet-4',
                  1000, 100, 0, 0, 0, 0, 10, 'success', ?, 'user', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(user_id.to_string())
    .bind(cost_micros)
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed spend");
}

#[tokio::test]
async fn an_admin_creates_a_member_allowance_and_reads_its_standing() {
    let app = test_app().await;
    let project_id = seed_project(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;
    seed_spend(&app, project_id, member.id, 90_000_000).await;

    let (status, created) = app
        .put(
            "/api/spend-limits",
            Some(&admin),
            json!({
                "scope": "member",
                "subject_id": member.id,
                "period": "month",
                "limit_usd_micros": 100_000_000u64,
                "warn_percent": 80
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["subject_label"], member.display_name);

    let (status, listed) = app.get("/api/spend-limits", Some(&admin)).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let limits = listed["limits"].as_array().expect("limits");
    assert_eq!(limits.len(), 1);
    let status_body = &limits[0]["status"];
    assert_eq!(status_body["spent_usd_micros"], 90_000_000);
    assert_eq!(status_body["used_percent"], 90);
    assert_eq!(
        status_body["state"], "warning",
        "90 of 100 dollars is past the 80% threshold"
    );
    assert_eq!(
        limits[0]["subject_label"], member.display_name,
        "a list of raw ids is unreadable in a console"
    );
}

/// The silent failure this guards against: a limit that matches nothing looks
/// identical to a member who has not spent.
#[tokio::test]
async fn an_allowance_aimed_at_nobody_is_refused() {
    let app = test_app().await;
    seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;

    let cases = [
        (
            json!({"scope": "member", "subject_id": Uuid::new_v4(),
                   "period": "day", "limit_usd_micros": 1_000_000u64}),
            StatusCode::NOT_FOUND,
            "a member id nobody holds",
        ),
        (
            json!({"scope": "member", "subject_id": "someone@example.test",
                   "period": "day", "limit_usd_micros": 1_000_000u64}),
            StatusCode::BAD_REQUEST,
            "an email where an id belongs",
        ),
        (
            json!({"scope": "role", "subject_id": "contributor",
                   "period": "day", "limit_usd_micros": 1_000_000u64}),
            StatusCode::BAD_REQUEST,
            "a role that is not a role",
        ),
        (
            json!({"scope": "role", "period": "day", "limit_usd_micros": 1_000_000u64}),
            StatusCode::BAD_REQUEST,
            "a role limit with no subject",
        ),
        (
            json!({"scope": "project", "subject_id": "everyone",
                   "period": "day", "limit_usd_micros": 1_000_000u64}),
            StatusCode::BAD_REQUEST,
            "a project limit given a subject",
        ),
        (
            json!({"scope": "project", "period": "day",
                   "limit_usd_micros": 1_000_000u64, "warn_percent": 800}),
            StatusCode::BAD_REQUEST,
            "a threshold no warning could ever reach",
        ),
    ];
    for (body, expected, reason) in cases {
        let (status, response) = app.put("/api/spend-limits", Some(&admin), body).await;
        assert_eq!(status, expected, "{reason}: {response}");
    }

    let (_, listed) = app.get("/api/spend-limits", Some(&admin)).await;
    assert_eq!(
        listed["limits"].as_array().map(Vec::len),
        Some(0),
        "a refused request must leave nothing behind"
    );
}

/// `PUT` replaces rather than patches, and a disabled limit stays visible
/// while no longer being evaluated.
#[tokio::test]
async fn writing_the_same_allowance_again_replaces_it() {
    let app = test_app().await;
    let project_id = seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;
    seed_spend(
        &app,
        project_id,
        app.seed_user(PrimaryRole::User).await.id,
        5_000_000,
    )
    .await;

    let body = |limit: u64, enabled: bool| {
        json!({
            "scope": "project",
            "period": "month",
            "limit_usd_micros": limit,
            "enabled": enabled
        })
    };
    let (status, _) = app
        .put("/api/spend-limits", Some(&admin), body(10_000_000, true))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, replaced) = app
        .put("/api/spend-limits", Some(&admin), body(50_000_000, false))
        .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(
        replaced["warn_percent"], 80,
        "an omitted threshold returns to the default, because this is a replace"
    );

    let (_, listed) = app.get("/api/spend-limits", Some(&admin)).await;
    let limits = listed["limits"].as_array().expect("limits");
    assert_eq!(limits.len(), 1, "replaced, not duplicated");
    assert_eq!(limits[0]["limit_usd_micros"], 50_000_000);
    assert_eq!(limits[0]["enabled"], false);
    assert!(
        limits[0]["status"].is_null(),
        "a disabled limit is configured but not evaluated, which is not the \
         same as evaluating to zero"
    );
}

#[tokio::test]
async fn deleting_names_the_allowance_and_repeating_it_is_not_an_error() {
    let app = test_app().await;
    seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;
    let (status, _) = app
        .put(
            "/api/spend-limits",
            Some(&admin),
            json!({"scope": "role", "subject_id": "admin", "period": "day",
                   "limit_usd_micros": 40_000_000u64}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let path = "/api/spend-limits?scope=role&subject_id=admin&period=day";
    let (status, first) = app.delete(path, Some(&admin), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["removed"], true);

    let (status, again) = app.delete(path, Some(&admin), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(
        again["removed"], false,
        "gone already is a fact to report, not a failure"
    );

    let (status, bad) = app
        .delete(
            "/api/spend-limits?scope=role&period=day",
            Some(&admin),
            json!({}),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a role delete without a subject would be ambiguous: {bad}"
    );
}

/// Catalog status has to say why pricing is quiet, or an operator reads an
/// unpriced project as a free one.
#[tokio::test]
async fn model_pricing_status_reports_a_server_that_never_synced() {
    let app = test_app().await;
    seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;

    let (status, body) = app.get("/api/model-pricing", Some(&admin)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["catalog"].is_null());
    assert_eq!(body["priced_models"], 0);
    assert_eq!(
        body["sync_enabled"], false,
        "the fixture keeps the suite off models.dev"
    );

    let (status, refused) = app
        .post("/api/model-pricing/sync", Some(&admin), json!({}))
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a disabled sync says so rather than reaching upstream: {refused}"
    );
}

#[tokio::test]
async fn repricing_over_http_reports_the_same_counters_the_command_does() {
    let app = test_app().await;
    let project_id = seed_project(&app).await;
    let admin = app.token_for_role(PrimaryRole::Admin).await;
    conductor_server::core::model_pricing::apply_catalog(
        &app.state.db,
        br#"{"anthropic":{"id":"anthropic","models":{"claude-sonnet-4":{
            "id":"claude-sonnet-4","cost":{"input":3,"output":15}}}}}"#,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");

    let member = app.seed_user(PrimaryRole::User).await;
    sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, project_id, user_id, request_id, event_type, sequence,
            provider, model, tokens_in, tokens_out, cache_read_tokens,
            cache_write_tokens, reasoning_tokens, tool_use_tokens, duration_ms,
            status, primary_role_snapshot, reported_at, received_at
        ) VALUES (?, ?, ?, 'req', 'model_call', 1, 'anthropic', 'claude-sonnet-4',
                  1000000, 0, 0, 0, 0, 0, 10, 'success', 'user', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(member.id.to_string())
    .bind(Utc::now().to_rfc3339())
    .bind(Utc::now().to_rfc3339())
    .execute(app.state.db.pool())
    .await
    .expect("seed unpriced call");

    let (status, report) = app
        .post("/api/model-pricing/reprice", Some(&admin), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["examined"], 1);
    assert_eq!(
        report["priced_in_force"], 1,
        "a million input tokens at $3 per million"
    );
    assert_eq!(report["left_unpriced"], 0);

    let (_, again) = app
        .post("/api/model-pricing/reprice", Some(&admin), json!({}))
        .await;
    assert_eq!(
        again["examined"], 0,
        "repricing only ever fills a cost that is still absent"
    );
}
