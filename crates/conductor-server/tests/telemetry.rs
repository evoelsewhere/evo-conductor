mod support;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{
    ClientPlatform, PrimaryRole, SecretScope, TelemetryEventStatus, TelemetryEventType,
    TelemetryToolCategory, User,
};
use serde_json::{json, Value};
use support::{test_app, TestApp};
use uuid::Uuid;

async fn seed_instance(app: &TestApp) {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, bind_host, bind_port, collection_level,
            setup_completed, jwt_secret, created_at, updated_at
        ) VALUES (?, 'Telemetry test', '127.0.0.1', 4700, 'L1', 1, 'unused', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed instance");
}

async fn seed_connection_token(app: &TestApp, user: &User, raw: &str) {
    app.state
        .db
        .secrets()
        .insert(
            user.id,
            "Telemetry",
            "evc_tele",
            &hash_token(raw),
            &[
                SecretScope::SubscribeResources,
                SecretScope::ReportTelemetry,
            ],
            None,
        )
        .await
        .expect("seed token");
}

async fn register(app: &TestApp, raw: &str) -> String {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Idempotency-Key",
        HeaderValue::from_str(&Uuid::new_v4().to_string()).expect("idempotency header"),
    );
    let (status, body) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(raw),
            headers,
            json!({
                "installation_key": Uuid::new_v4(),
                "display_name": "Telemetry desktop",
                "platform": ClientPlatform::Macos,
                "evoflux_version": "0.8.0"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["installation"]["id"]
        .as_str()
        .expect("installation id")
        .to_string()
}

fn event_batch(installation_id: &str, request_id: Uuid) -> Value {
    let reported_at = chrono::Utc::now().to_rfc3339();
    json!({
        "installation_id": installation_id,
        "events": [
            {
                "event_id": Uuid::new_v4(),
                "request_id": request_id,
                "session_id": "session-1",
                "event_type": TelemetryEventType::ModelCall,
                "sequence": 1,
                "agent_name": "lead",
                "provider": "openai",
                "model": "gpt-5",
                "tokens_in": 100,
                "tokens_out": 50,
                "cache_read_tokens": 20,
                "reasoning_tokens": 10,
                "tool_use_tokens": 0,
                "duration_ms": 800,
                "tool_name": null,
                "tool_category": null,
                "status": TelemetryEventStatus::Success,
                "error_category": null,
                "reported_at": reported_at
            },
            {
                "event_id": Uuid::new_v4(),
                "request_id": request_id,
                "session_id": "session-1",
                "event_type": TelemetryEventType::ToolCall,
                "sequence": 2,
                "agent_name": "lead",
                "provider": null,
                "model": null,
                "tokens_in": 0,
                "tokens_out": 0,
                "cache_read_tokens": 0,
                "reasoning_tokens": 0,
                "tool_use_tokens": 0,
                "duration_ms": 125,
                "tool_name": "read_file",
                "tool_category": TelemetryToolCategory::Filesystem,
                "status": TelemetryEventStatus::Success,
                "error_category": null,
                "reported_at": reported_at
            }
        ]
    })
}

#[tokio::test]
async fn telemetry_is_idempotent_private_and_queryable_by_member() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_member";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;
    let request_id = Uuid::new_v4();
    let batch = event_batch(&installation_id, request_id);

    let (status, first) = app
        .post("/api/v1/telemetry/batch", Some(raw), batch.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["accepted"], 2);
    assert_eq!(first["duplicates"], 0);

    let (status, replay) = app.post("/api/v1/telemetry/batch", Some(raw), batch).await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["accepted"], 0);
    assert_eq!(replay["duplicates"], 2);

    let browser_token = app.token_for(&member).await;
    let (status, summary) = app
        .get(
            &format!("/api/members/{}/usage/summary", member.id),
            Some(&browser_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["total_requests"], 1);
    assert_eq!(summary["total_tokens"], 150);
    assert_eq!(summary["tool_calls"], 1);
    assert_eq!(summary["models"][0]["model"], "gpt-5");

    let (status, activity) = app
        .get(
            &format!("/api/members/{}/activity", member.id),
            Some(&browser_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    assert_eq!(activity["total"], 1);
    assert_eq!(activity["items"][0]["total_tokens"], 150);

    let (status, detail) = app
        .get(
            &format!("/api/members/{}/activity/{}", member.id, request_id),
            Some(&browser_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["events"].as_array().map(Vec::len), Some(2));
    assert!(detail.to_string().find("prompt").is_none());

    let (status, tools) = app
        .get(
            &format!("/api/members/{}/tools", member.id),
            Some(&browser_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{tools}");
    assert_eq!(tools["total_calls"], 1);
    assert_eq!(tools["tools"][0]["tool_name"], "read_file");

    let other = app.seed_user(PrimaryRole::User).await;
    let other_browser_token = app.token_for(&other).await;
    let (status, _) = app
        .get(
            &format!("/api/members/{}/activity", member.id),
            Some(&other_browser_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let admin_token = app.token_for_role(PrimaryRole::Admin).await;
    let (status, _) = app
        .get(
            &format!("/api/members/{}/activity", member.id),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn telemetry_rejects_sensitive_or_cross_owner_payloads() {
    let app = test_app().await;
    seed_instance(&app).await;
    let owner = app.seed_user(PrimaryRole::User).await;
    let owner_raw = "evc_telemetry_owner";
    seed_connection_token(&app, &owner, owner_raw).await;
    let installation_id = register(&app, owner_raw).await;

    let mut sensitive = event_batch(&installation_id, Uuid::new_v4());
    sensitive["events"][0]["prompt"] = json!("must never be accepted");
    let (status, _) = app
        .post("/api/v1/telemetry/batch", Some(owner_raw), sensitive)
        .await;
    assert!(matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
    ));

    let owner_browser_token = app.token_for(&owner).await;
    let (status, empty) = app
        .get(
            &format!("/api/members/{}/usage/summary", owner.id),
            Some(&owner_browser_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["total_requests"], 0);
    assert_eq!(empty["total_tokens"], 0);

    let other = app.seed_user(PrimaryRole::User).await;
    let other_raw = "evc_telemetry_other";
    seed_connection_token(&app, &other, other_raw).await;
    let (status, _) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(other_raw),
            event_batch(&installation_id, Uuid::new_v4()),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
