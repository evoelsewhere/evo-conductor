mod support;

use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{
    AuthorizationAction, ClientPlatform, DecisionReason, PrimaryRole, SecretScope,
    TelemetryBatchRequest, TelemetryEventStatus, TelemetryEventType, User,
};
use conductor_server::core::authorization::{
    AuthorizationDecisionObserver, AuthorizationEvent, AuthorizationResult, AuthorizationService,
    AuthorizationStage,
};
use serde_json::{json, Value};
use sqlx::Row;
use support::{test_app, test_app_with_authorization, TestApp};
use uuid::Uuid;

#[derive(Default)]
struct RecordingObserver(Mutex<Vec<AuthorizationEvent>>);

impl AuthorizationDecisionObserver for RecordingObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        self.0.lock().expect("observer lock").push(event.clone());
    }
}

async fn seed_instance(app: &TestApp) -> Uuid {
    let now = chrono::Utc::now().to_rfc3339();
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, bind_host, bind_port, collection_level,
            setup_completed, jwt_secret, created_at, updated_at
        ) VALUES (?, 'Telemetry test', '127.0.0.1', 4700, 'L1', 1, 'unused', ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed instance");
    id
}

async fn seed_resource(
    app: &TestApp,
    project_id: Uuid,
    owner_id: Uuid,
    kind: &str,
    slug: &str,
    name: &str,
) -> (Uuid, Uuid) {
    let now = chrono::Utc::now().to_rfc3339();
    let resource_id = Uuid::new_v4();
    let version_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO resources (
            id, project_id, kind, slug, name, version, owner_user_id, visibility,
            status, payload, draft_revision, highest_semver, release_channel,
            published_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, '1.2.0', ?, 'shared',
                  'published', '{}', 0, '1.2.0', 'published', ?, ?, ?)
        "#,
    )
    .bind(resource_id.to_string())
    .bind(project_id.to_string())
    .bind(kind)
    .bind(slug)
    .bind(name)
    .bind(owner_id.to_string())
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed resource");
    sqlx::query(
        r#"
        INSERT INTO resource_versions (
            id, project_id, resource_id, version, status, payload, release_channel,
            content_sha256, content_size, created_by, created_at, published_at
        ) VALUES (?, ?, ?, '1.2.0', 'published', '{}', 'published', 'abc', 2, ?, ?, ?)
        "#,
    )
    .bind(version_id.to_string())
    .bind(project_id.to_string())
    .bind(resource_id.to_string())
    .bind(owner_id.to_string())
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed resource version");
    (resource_id, version_id)
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
                SecretScope::SyncInventory,
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
    assert!(first.get("summary").is_none());

    let (status, replay) = app.post("/api/v1/telemetry/batch", Some(raw), batch).await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["accepted"], 0);
    assert_eq!(replay["duplicates"], 2);
    assert!(replay.get("summary").is_none());

    let (status, refresh) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            json!({"installation_id": installation_id, "events": []}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{refresh}");
    assert_eq!(refresh["accepted"], 0);
    assert_eq!(refresh["duplicates"], 0);
    assert_eq!(refresh["summary"]["events"], 2);
    assert_eq!(refresh["summary"]["requests"], 1);
    assert_eq!(refresh["summary"]["model_calls"], 1);
    assert_eq!(refresh["summary"]["tool_calls"], 1);
    assert_eq!(refresh["summary"]["attributed_events"], 0);
    assert_eq!(refresh["summary"]["window_days"], 30);

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

    let second_installation_id = register(&app, raw).await;
    let (status, second_installation) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            event_batch(&second_installation_id, Uuid::new_v4()),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{second_installation}");
    assert_eq!(second_installation["accepted"], 2);

    let (status, first_installation) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            json!({"installation_id": installation_id, "events": []}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{first_installation}");
    assert_eq!(first_installation["summary"]["events"], 2);

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
    let project_id = seed_instance(&app).await;
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

    let (plugin_id, plugin_version_id) = seed_resource(
        &app,
        project_id,
        owner.id,
        "plugin",
        "managed-plugin",
        "Managed plugin",
    )
    .await;
    let mut forged_plugin = event_batch(&installation_id, Uuid::new_v4());
    forged_plugin["events"][1]["resources"] = json!([{
        "resource_id": plugin_id,
        "version_id": plugin_version_id,
        "relation": "plugin_contributed_tool",
        "plugin_installation_id": "forged-installation"
    }]);
    let (status, _) = app
        .post("/api/v1/telemetry/batch", Some(owner_raw), forged_plugin)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

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

/// Before these caps existed the wire type was the only bound, so one event
/// reporting `u64::MAX` saturated to `i64::MAX` at insert and permanently
/// wrecked every aggregate over that member. The boundary cases matter as much
/// as the rejections: a cap that also rejects the legal maximum would stall a
/// well-behaved client's outbox.
#[tokio::test]
async fn telemetry_rejects_absurd_token_cost_and_duration_values() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_caps";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    for field in [
        "tokens_in",
        "tokens_out",
        "cache_read_tokens",
        "reasoning_tokens",
        "tool_use_tokens",
    ] {
        let mut batch = event_batch(&installation_id, Uuid::new_v4());
        batch["events"][0][field] = json!(u64::MAX);
        let (status, body) = app.post("/api/v1/telemetry/batch", Some(raw), batch).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{field} at u64::MAX must be rejected, got {body}"
        );

        let mut over = event_batch(&installation_id, Uuid::new_v4());
        over["events"][0][field] = json!(100_000_001u64);
        let (status, body) = app.post("/api/v1/telemetry/batch", Some(raw), over).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{field} one over the cap must be rejected, got {body}"
        );

        let mut at_cap = event_batch(&installation_id, Uuid::new_v4());
        at_cap["events"][0][field] = json!(100_000_000u64);
        let (status, body) = app.post("/api/v1/telemetry/batch", Some(raw), at_cap).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{field} exactly at the cap must be accepted, got {body}"
        );
    }

    let mut long = event_batch(&installation_id, Uuid::new_v4());
    long["events"][0]["duration_ms"] = json!(86_400_001u64);
    let (status, body) = app.post("/api/v1/telemetry/batch", Some(raw), long).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn inventory_rejects_unknown_state_and_cross_resource_versions() {
    let app = test_app().await;
    let project_id = seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let owner = app.seed_user(PrimaryRole::Contribute).await;
    let (first_resource_id, first_version_id) =
        seed_resource(&app, project_id, owner.id, "agent_team", "first", "First").await;
    let (_second_resource_id, second_version_id) =
        seed_resource(&app, project_id, owner.id, "skill", "second", "Second").await;
    let raw = "evc_invalid_inventory";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;
    let observed_at = chrono::Utc::now().to_rfc3339();

    let (status, _) = app
        .put(
            "/api/v1/client/inventory",
            Some(raw),
            json!({
                "installation_id": installation_id,
                "items": [{
                    "resource_id": first_resource_id,
                    "desired_version_id": first_version_id,
                    "applied_version_id": first_version_id,
                    "release_channel": "published",
                    "content_sha256": "abc",
                    "plugin_installation_id": null,
                    "observed_state": "made_up_state",
                    "error_category": null,
                    "observed_at": observed_at
                }]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, body) = app
        .put(
            "/api/v1/client/inventory",
            Some(raw),
            json!({
                "installation_id": installation_id,
                "items": [{
                    "resource_id": first_resource_id,
                    "desired_version_id": first_version_id,
                    "applied_version_id": second_version_id,
                    "release_channel": "published",
                    "content_sha256": "abc",
                    "plugin_installation_id": null,
                    "observed_state": "applied",
                    "error_category": null,
                    "observed_at": observed_at
                }]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM installation_resource_inventory WHERE installation_id = ?",
    )
    .bind(installation_id)
    .fetch_one(app.state.db.pool())
    .await
    .expect("count rejected inventory rows");
    assert_eq!(count, 0);
}

/// A plugin version update mints a brand-new plugin_installation_id and the
/// client re-syncs inventory to the new (version, installation) pair, but
/// telemetry generated moments earlier -- and still sitting in the client's
/// outbox when the update lands -- still names the just-superseded pair.
/// That telemetry must still validate once, not be permanently poisoned.
#[tokio::test]
async fn telemetry_still_validates_against_the_plugin_installation_a_version_update_just_replaced() {
    let app = test_app().await;
    let project_id = seed_instance(&app).await;
    let owner = app.seed_user(PrimaryRole::User).await;
    let (resource_id, version_id_a) =
        seed_resource(&app, project_id, owner.id, "plugin", "jira-plugin", "Jira plugin").await;
    let version_id_b = Uuid::new_v4();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO resource_versions (
            id, project_id, resource_id, version, status, payload, release_channel,
            content_sha256, content_size, created_by, created_at, published_at
        ) VALUES (?, ?, ?, '1.3.0', 'published', '{}', 'published', 'def', 2, ?, ?, ?)
        "#,
    )
    .bind(version_id_b.to_string())
    .bind(project_id.to_string())
    .bind(resource_id.to_string())
    .bind(owner.id.to_string())
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed second resource version");

    let raw = "evc_plugin_cutover";
    seed_connection_token(&app, &owner, raw).await;
    let installation_id = register(&app, raw).await;

    let inventory_item = |version_id: Uuid, plugin_installation_id: &str| {
        json!({
            "installation_id": installation_id,
            "items": [{
                "resource_id": resource_id,
                "desired_version_id": version_id,
                "applied_version_id": version_id,
                "release_channel": "published",
                "content_sha256": if version_id == version_id_a { "abc" } else { "def" },
                "plugin_installation_id": plugin_installation_id,
                "observed_state": "applied",
                "error_category": null,
                "observed_at": chrono::Utc::now().to_rfc3339()
            }]
        })
    };
    let telemetry_referencing = |version_id: Uuid, plugin_installation_id: &str| {
        let mut batch = event_batch(&installation_id, Uuid::new_v4());
        batch["events"][1]["resources"] = json!([{
            "resource_id": resource_id,
            "version_id": version_id,
            "relation": "plugin_contributed_tool",
            "plugin_installation_id": plugin_installation_id
        }]);
        batch
    };

    let (status, _) = app
        .put(
            "/api/v1/client/inventory",
            Some(raw),
            inventory_item(version_id_a, "install-a"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            telemetry_referencing(version_id_a, "install-a"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "current generation must validate");

    // The version update: the client re-syncs inventory under a new
    // installation id before its outbox has drained the events above.
    let (status, _) = app
        .put(
            "/api/v1/client/inventory",
            Some(raw),
            inventory_item(version_id_b, "install-b"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            telemetry_referencing(version_id_a, "install-a"),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "telemetry from the just-superseded installation must still validate once"
    );

    let (status, _) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            telemetry_referencing(version_id_b, "install-b"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "the new generation must validate too");

    let (status, _) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            telemetry_referencing(version_id_a, "install-never-recorded"),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a generation older than the one just replaced must still be rejected"
    );
}

#[tokio::test]
async fn resource_usage_analytics_attributes_member_role_version_tokens_and_cost() {
    let observer = Arc::new(RecordingObserver::default());
    let app = test_app_with_authorization(AuthorizationService::new(observer.clone())).await;
    let project_id = seed_instance(&app).await;
    // Cost has to come from Conductor's catalog now — a client cannot supply
    // one — and this test needs a non-zero figure to show the double count.
    conductor_server::core::model_pricing::apply_catalog(
        &app.state.db,
        br#"{"openai":{"id":"openai","models":{
            "gpt-5":{"id":"gpt-5","cost":{"input":3,"output":15}}}},
            "anthropic":{"id":"anthropic","models":{
            "claude-sonnet-4":{"id":"claude-sonnet-4","cost":{"input":3,"output":15}}}}}"#,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");
    app.state
        .model_rates
        .reload(&app.state.db)
        .await
        .expect("load rates");
    let member = app.seed_user(PrimaryRole::User).await;
    let resource_owner = app.seed_user(PrimaryRole::Contribute).await;
    let (resource_id, version_id) = seed_resource(
        &app,
        project_id,
        resource_owner.id,
        "agent_team",
        "reviewer",
        "Reviewer",
    )
    .await;
    let (skill_id, skill_version_id) = seed_resource(
        &app,
        project_id,
        resource_owner.id,
        "skill",
        "release-check",
        "Release check",
    )
    .await;
    let raw = "evc_resource_telemetry";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;
    let observed_at = chrono::Utc::now().to_rfc3339();
    let (status, inventory_response) = app
        .put(
            "/api/v1/client/inventory",
            Some(raw),
            json!({
                "installation_id": installation_id,
                "items": [
                    {
                        "resource_id": resource_id,
                        "desired_version_id": version_id,
                        "applied_version_id": version_id,
                        "release_channel": "published",
                        "content_sha256": "abc",
                        "plugin_installation_id": null,
                        "observed_state": "in_sync",
                        "error_category": null,
                        "observed_at": observed_at
                    },
                    {
                        "resource_id": skill_id,
                        "desired_version_id": skill_version_id,
                        "applied_version_id": skill_version_id,
                        "release_channel": "published",
                        "content_sha256": "abc",
                        "plugin_installation_id": null,
                        "observed_state": "applied",
                        "error_category": null,
                        "observed_at": observed_at
                    }
                ]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{inventory_response}");
    assert_eq!(inventory_response["accepted"], 2);
    let request_id = Uuid::new_v4();
    let mut batch = event_batch(&installation_id, request_id);
    let agent_reference = json!({
        "resource_id": resource_id,
        "version_id": version_id,
        "relation": "executing_agent",
        "plugin_installation_id": null
    });
    let skill_reference = json!({
        "resource_id": skill_id,
        "version_id": skill_version_id,
        "relation": "activated_skill",
        "plugin_installation_id": null
    });
    for event in batch["events"].as_array_mut().expect("events") {
        event["resources"] = json!([agent_reference.clone(), skill_reference.clone()]);
    }
    batch["events"].as_array_mut().expect("events").push(json!({
        "event_id": Uuid::new_v4(),
        "request_id": request_id,
        "session_id": "session-1",
        "event_type": "request",
        "sequence": 3,
        "agent_name": "reviewer",
        "provider": null,
        "model": null,
        "tokens_in": 0,
        "tokens_out": 0,
        "duration_ms": 1000,
        "status": "success",
        "error_category": null,
        "resources": [agent_reference, skill_reference],
        "reported_at": chrono::Utc::now().to_rfc3339()
    }));
    let plain_request_id = Uuid::new_v4();
    batch["events"].as_array_mut().expect("events").push(json!({
        "event_id": Uuid::new_v4(),
        "request_id": plain_request_id,
        "session_id": "session-plain",
        "event_type": "model_call",
        "sequence": 1,
        "agent_name": "evoflux",
        "provider": "anthropic",
        "model": "claude-sonnet-4",
        "tokens_in": 300,
        "tokens_out": 100,
        "cache_read_tokens": 60,
        "reasoning_tokens": 20,
        "tool_use_tokens": 0,
        "duration_ms": 600,
        "status": "success",
        "error_category": null,
        "resources": [],
        "reported_at": chrono::Utc::now().to_rfc3339()
    }));
    batch["events"].as_array_mut().expect("events").push(json!({
        "event_id": Uuid::new_v4(),
        "request_id": plain_request_id,
        "session_id": "session-plain",
        "event_type": "tool_call",
        "sequence": 2,
        "agent_name": "evoflux",
        "provider": null,
        "model": null,
        "tokens_in": 0,
        "tokens_out": 0,
        "duration_ms": 80,
        "status": "success",
        "error_category": null,
        "resources": [],
        "reported_at": chrono::Utc::now().to_rfc3339()
    }));
    batch["events"].as_array_mut().expect("events").push(json!({
        "event_id": Uuid::new_v4(),
        "request_id": plain_request_id,
        "session_id": "session-plain",
        "event_type": "request",
        "sequence": 3,
        "agent_name": null,
        "provider": null,
        "model": null,
        "tokens_in": 0,
        "tokens_out": 0,
        "duration_ms": 250,
        "status": "success",
        "error_category": null,
        "resources": [],
        "reported_at": chrono::Utc::now().to_rfc3339()
    }));
    serde_json::from_value::<TelemetryBatchRequest>(batch.clone()).expect("valid telemetry batch");

    let (status, response) = app.post("/api/v1/telemetry/batch", Some(raw), batch).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["accepted"], 6);
    let (status, receipt) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            json!({"installation_id": installation_id, "events": []}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["summary"]["events"], 6);
    assert_eq!(receipt["summary"]["requests"], 2);
    assert_eq!(receipt["summary"]["attributed_events"], 3);
    assert_eq!(receipt["summary"]["attributed_requests"], 1);
    assert_eq!(receipt["summary"]["attributed_model_calls"], 1);
    assert_eq!(receipt["summary"]["attributed_tool_calls"], 1);

    let admin_token = app.token_for_role(PrimaryRole::Admin).await;
    let (status, analytics) = app
        .get("/api/analytics/resource-usage", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{analytics}");
    assert_eq!(analytics["scope"], "governed");
    assert_eq!(analytics["totals"]["all_requests"], 2);
    assert_eq!(analytics["totals"]["governed_requests"], 1);
    assert_eq!(analytics["totals"]["requests"], 1);
    assert_eq!(analytics["totals"]["resource_uses"], 2);
    assert_eq!(analytics["totals"]["model_calls"], 1);
    assert_eq!(analytics["totals"]["cache_read_tokens"], 20);
    assert_eq!(analytics["totals"]["reasoning_tokens"], 10);
    assert_eq!(analytics["totals"]["total_tokens"], 150);
    assert_eq!(analytics["totals"]["average_tokens_per_request"], 150);
    // 100 input x $3 + 50 output x $15, per million tokens.
    assert_eq!(analytics["totals"]["estimated_cost_usd_micros"], 1050);
    assert_eq!(analytics["totals"]["unpriced_model_calls"], 0);
    assert_eq!(analytics["totals"]["reported_installations"], 2);
    assert_eq!(analytics["totals"]["installed_installations"], 1);
    assert_eq!(analytics["totals"]["installed_members"], 1);
    assert_eq!(analytics["roles"][0]["primary_role"], "user");
    assert_eq!(analytics["roles"][0]["tool_calls"], 1);
    assert_eq!(analytics["roles"][0]["total_tokens"], 150);
    let resource_rows = analytics["resources"].as_array().expect("resource rows");
    assert_eq!(resource_rows.len(), 2);
    assert!(resource_rows
        .iter()
        .any(|row| row["resource_id"] == resource_id.to_string()));
    assert!(resource_rows.iter().all(|row| row["total_tokens"] == 150));
    // The one governed request was attributed to two resources, so it is
    // counted in full against each: these rows overlap on purpose. They rank
    // versions against one another; they do not partition the project total.
    // See `ResourceUsageBreakdown`.
    assert!(resource_rows
        .iter()
        .all(|row| row["estimated_cost_usd_micros"] == 1050));
    let summed_across_rows: i64 = resource_rows
        .iter()
        .filter_map(|row| row["estimated_cost_usd_micros"].as_i64())
        .sum();
    assert_eq!(
        summed_across_rows, 2100,
        "adding the rows up double-counts the request, which is why the UI must not present them as a breakdown of the total"
    );
    assert_eq!(
        analytics["totals"]["estimated_cost_usd_micros"], 1050,
        "while the total counts that request once"
    );
    assert_eq!(analytics["members"][0]["primary_role"], "user");
    assert_eq!(analytics["members"][0]["model_calls"], 1);
    assert_eq!(analytics["members"][0]["tool_calls"], 1);
    assert_eq!(analytics["members"][0]["installations"], 1);
    assert_eq!(analytics["members"][0]["total_tokens"], 150);
    assert!(analytics["members"][0]["last_received_at"].is_string());
    assert_eq!(analytics["models"][0]["total_tokens"], 150);
    assert_eq!(analytics["activity"][0]["total_tokens"], 150);
    assert_eq!(
        analytics["activity"][0]["display_name"],
        member.display_name
    );

    let (status, all_analytics) = app
        .get(
            "/api/analytics/resource-usage?scope=all",
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{all_analytics}");
    assert_eq!(all_analytics["scope"], "all");
    assert_eq!(all_analytics["totals"]["all_requests"], 2);
    assert_eq!(all_analytics["totals"]["governed_requests"], 1);
    assert_eq!(all_analytics["totals"]["requests"], 2);
    assert_eq!(all_analytics["totals"]["model_calls"], 2);
    assert_eq!(all_analytics["totals"]["tool_calls"], 2);
    assert_eq!(all_analytics["totals"]["total_tokens"], 550);
    assert_eq!(all_analytics["totals"]["estimated_cost_usd_micros"], 3450);
    assert_eq!(all_analytics["members"][0]["model_calls"], 2);
    assert_eq!(all_analytics["members"][0]["tool_calls"], 2);
    assert_eq!(all_analytics["members"][0]["total_tokens"], 550);
    assert_eq!(all_analytics["models"].as_array().map(Vec::len), Some(2));
    assert!(all_analytics["models"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["model"] == "claude-sonnet-4")));
    assert_eq!(all_analytics["resources"].as_array().map(Vec::len), Some(0));
    assert_eq!(all_analytics["activity"].as_array().map(Vec::len), Some(0));
    assert_eq!(all_analytics["activity_total"], 0);

    let (status, invalid_scope_filter) = app
        .get(
            &format!("/api/analytics/resource-usage?scope=all&resource_id={resource_id}"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid_scope_filter}");

    let contributor_token = app.token_for(&resource_owner).await;
    let (status, contributor_analytics) = app
        .get("/api/analytics/resource-usage", Some(&contributor_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{contributor_analytics}");
    assert_eq!(contributor_analytics["totals"]["all_requests"], 2);
    assert_eq!(contributor_analytics["totals"]["requests"], 1);
    assert_eq!(
        contributor_analytics["members"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        contributor_analytics["activity"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(contributor_analytics["activity_total"], 0);
    let serialized = contributor_analytics.to_string();
    assert!(!serialized.contains(&member.id.to_string()));
    assert!(!serialized.contains(&member.email));

    let (status, contributor_all) = app
        .get(
            "/api/analytics/resource-usage?scope=all",
            Some(&contributor_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{contributor_all}");
    assert_eq!(contributor_all["scope"], "all");
    assert_eq!(contributor_all["totals"]["requests"], 2);
    assert_eq!(contributor_all["members"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        contributor_all["activity"].as_array().map(Vec::len),
        Some(0)
    );
    assert!(!contributor_all.to_string().contains(&member.email));

    for identifying_filter in [
        format!("member_id={}", member.id),
        format!("installation_id={installation_id}"),
    ] {
        let (status, _) = app
            .get(
                &format!("/api/analytics/resource-usage?{identifying_filter}"),
                Some(&contributor_token),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    {
        let events = observer.0.lock().expect("observer lock");
        assert!(events.iter().any(|event| {
            event.stage == AuthorizationStage::Target
                && event.action == AuthorizationAction::AnalyticsResourceUsageRead
                && event.actor_id == resource_owner.id
                && event.authorization_result == AuthorizationResult::Denied
                && event.reason_code == Some(DecisionReason::DenyDetailAccess)
        }));
    }

    let (status, _) = app
        .get(
            &format!("/api/members/{}/activity", member.id),
            Some(&contributor_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, filtered) = app
        .get(
            "/api/analytics/resource-usage?provider=openai&model=gpt-5&status=success",
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{filtered}");
    assert_eq!(filtered["totals"]["all_requests"], 1);
    assert_eq!(filtered["totals"]["requests"], 1);
    assert_eq!(filtered["totals"]["successes"], 1);
    assert_eq!(filtered["totals"]["model_calls"], 1);
    assert_eq!(filtered["totals"]["total_tokens"], 150);
    assert_eq!(filtered["activity"][0]["total_tokens"], 150);
    assert_eq!(filtered["activity"][0]["status"], "success");

    let (status, resource_inventory) = app
        .get(
            &format!("/api/resources/{resource_id}/inventory"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resource_inventory}");
    assert_eq!(resource_inventory["summary"]["installed_installations"], 1);
    assert_eq!(resource_inventory["summary"]["installed_members"], 1);
    assert_eq!(
        resource_inventory["installations"][0]["user_id"],
        member.id.to_string()
    );
    assert_eq!(
        resource_inventory["installations"][0]["desired_version"],
        "1.2.0"
    );
    assert_eq!(
        resource_inventory["installations"][0]["applied_version"],
        "1.2.0"
    );
    let (status, contributor_inventory) = app
        .get(
            &format!("/api/resources/{resource_id}/inventory"),
            Some(&contributor_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{contributor_inventory}");
    assert_eq!(
        contributor_inventory["summary"]["installed_installations"],
        1
    );
    assert_eq!(
        contributor_inventory["installations"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    let contributor_inventory_json = contributor_inventory.to_string();
    assert!(!contributor_inventory_json.contains(&member.id.to_string()));
    assert!(!contributor_inventory_json.contains(&member.email));

    let member_token = app.token_for(&member).await;
    let (status, _) = app
        .get(
            &format!("/api/resources/{resource_id}/inventory"),
            Some(&member_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, detail) = app
        .get(
            &format!("/api/members/{}/activity/{}", member.id, request_id),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["request"]["estimated_cost_usd_micros"], 1050);
    let model_event = detail["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| event["event_type"] == "model_call")
        .expect("model event");
    assert_eq!(model_event["resources"].as_array().map(Vec::len), Some(2));

    // Client request IDs are scoped by member. Two members may legitimately
    // emit the same opaque request ID and both must count in role analytics.
    let second_member = app.seed_user(PrimaryRole::User).await;
    let second_raw = "evc_resource_telemetry_second_member";
    seed_connection_token(&app, &second_member, second_raw).await;
    let second_installation_id = register(&app, second_raw).await;
    let mut second_batch = event_batch(&second_installation_id, request_id);
    let second_agent_reference = json!({
        "resource_id": resource_id,
        "version_id": version_id,
        "relation": "executing_agent",
        "plugin_installation_id": null
    });
    for event in second_batch["events"].as_array_mut().expect("events") {
        event["resources"] = json!([second_agent_reference.clone()]);
    }
    second_batch["events"]
        .as_array_mut()
        .expect("events")
        .push(json!({
            "event_id": Uuid::new_v4(),
            "request_id": request_id,
            "session_id": "session-2",
            "event_type": "request",
            "sequence": 3,
            "agent_name": "reviewer",
            "provider": null,
            "model": null,
            "tokens_in": 0,
            "tokens_out": 0,
            "duration_ms": 500,
            "status": "success",
            "error_category": null,
            "resources": [second_agent_reference],
            "reported_at": chrono::Utc::now().to_rfc3339()
        }));
    let (status, body) = app
        .post("/api/v1/telemetry/batch", Some(second_raw), second_batch)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, two_member_analytics) = app
        .get("/api/analytics/resource-usage", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{two_member_analytics}");
    assert_eq!(two_member_analytics["totals"]["all_requests"], 3);
    let user_role = two_member_analytics["roles"]
        .as_array()
        .expect("role rows")
        .iter()
        .find(|row| row["primary_role"] == "user")
        .expect("user role row");
    assert_eq!(user_role["requests"], 2);

    // The member row represents the current access profile; role aggregates
    // above intentionally retain the role captured when each event arrived.
    sqlx::query("UPDATE users SET primary_role = 'contribute' WHERE id = ?")
        .bind(member.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("change current member role");
    let (status, changed_role_analytics) = app
        .get("/api/analytics/resource-usage", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{changed_role_analytics}");
    let member_row = changed_role_analytics["members"]
        .as_array()
        .expect("member rows")
        .iter()
        .find(|row| row["user_id"] == member.id.to_string())
        .expect("changed member row");
    assert_eq!(member_row["primary_role"], "contribute");
    assert!(changed_role_analytics["roles"]
        .as_array()
        .expect("role rows")
        .iter()
        .any(|row| row["primary_role"] == "user"));

    sqlx::query(
        "UPDATE installation_resource_inventory SET observed_state = 'future_state' \
         WHERE installation_id = ? AND resource_id = ?",
    )
    .bind(&installation_id)
    .bind(resource_id.to_string())
    .execute(app.state.db.pool())
    .await
    .expect("corrupt monitored inventory state");
    let (status, body) = app
        .get(
            &format!("/api/resources/{resource_id}/inventory"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(!body.to_string().contains("future_state"));
    sqlx::query(
        "UPDATE installation_resource_inventory SET observed_state = 'in_sync' \
         WHERE installation_id = ? AND resource_id = ?",
    )
    .bind(&installation_id)
    .bind(resource_id.to_string())
    .execute(app.state.db.pool())
    .await
    .expect("restore monitored inventory state");

    sqlx::query("UPDATE users SET primary_role = 'future_role' WHERE id = ?")
        .bind(member.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("corrupt monitored member role");
    let (status, body) = app
        .get(
            &format!("/api/resources/{resource_id}/inventory"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(!body.to_string().contains("future_role"));
}

/// End-to-end proof of server-side pricing: with a catalog synced, Conductor
/// must store its own cost for a model call and keep the client's figure
/// beside it. Without the second half, a client on a stale rate table would
/// diverge from the project's reported spend invisibly.
#[tokio::test]
async fn ingest_prices_model_calls_from_conductors_own_catalog() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_priced";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    let payload = br#"{
        "anthropic": {
            "id": "anthropic",
            "models": {
                "claude-sonnet-4": {
                    "id": "claude-sonnet-4",
                    "cost": {"input": 3, "output": 15, "cache_read": 0.3}
                }
            }
        }
    }"#;
    conductor_server::core::model_pricing::apply_catalog(
        &app.state.db,
        payload,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");
    app.state
        .model_rates
        .reload(&app.state.db)
        .await
        .expect("load rates");

    let event_id = Uuid::new_v4();
    let batch = json!({
        "installation_id": installation_id,
        "events": [{
            "event_id": event_id,
            "request_id": Uuid::new_v4(),
            "event_type": TelemetryEventType::ModelCall,
            "sequence": 1,
            // Mixed casing on purpose: the catalog is keyed normalized.
            "provider": "Anthropic",
            "model": "Claude-Sonnet-4",
            "tokens_in": 1_000_000,
            "tokens_out": 200_000,
            "cache_read_tokens": 400_000,
            "reasoning_tokens": 0,
            "tool_use_tokens": 0,
            "duration_ms": 900,
            "status": TelemetryEventStatus::Success,
            // Deliberately wrong, to prove Conductor does not echo it.
            "reported_at": chrono::Utc::now().to_rfc3339()
        }]
    });
    let (status, body) = app.post("/api/v1/telemetry/batch", Some(raw), batch).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let row = sqlx::query(
        "SELECT estimated_cost_usd_micros, server_cost_usd_micros, priced_catalog_version, \
         unpriced_reason FROM telemetry_events WHERE id = ?",
    )
    .bind(event_id.to_string())
    .fetch_one(app.state.db.pool())
    .await
    .expect("read the stored event");

    // input:   600,000 x $3    = $1.80  (cache reads carved out)
    // cache_r: 400,000 x $0.30 = $0.12
    // output:  200,000 x $15   = $3.00
    let expected = 1_800_000 + 120_000 + 3_000_000;
    assert_eq!(
        row.get::<Option<i64>, _>("server_cost_usd_micros"),
        Some(expected),
        "Conductor must price from its own rate table"
    );
    assert_eq!(
        row.get::<Option<i64>, _>("estimated_cost_usd_micros"),
        None,
        "a cost sent by a client is discarded, never stored alongside          Conductor's own"
    );
    assert!(row
        .get::<Option<String>, _>("priced_catalog_version")
        .is_some());
    assert_eq!(row.get::<Option<String>, _>("unpriced_reason"), None);
}

/// A model absent from the catalog must record *why* it is unpriced rather
/// than recording a zero cost, which would read as free usage.
#[tokio::test]
async fn an_unpriced_model_records_a_reason_instead_of_zero() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_unpriced";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    let event_id = Uuid::new_v4();
    let batch = json!({
        "installation_id": installation_id,
        "events": [{
            "event_id": event_id,
            "request_id": Uuid::new_v4(),
            "event_type": TelemetryEventType::ModelCall,
            "sequence": 1,
            "provider": "who-knows",
            "model": "unlisted-model",
            "tokens_in": 1000,
            "tokens_out": 500,
            "cache_read_tokens": 0,
            "reasoning_tokens": 0,
            "tool_use_tokens": 0,
            "duration_ms": 10,
            "status": TelemetryEventStatus::Success,
            "reported_at": chrono::Utc::now().to_rfc3339()
        }]
    });
    let (status, body) = app.post("/api/v1/telemetry/batch", Some(raw), batch).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let row = sqlx::query(
        "SELECT server_cost_usd_micros, unpriced_reason FROM telemetry_events WHERE id = ?",
    )
    .bind(event_id.to_string())
    .fetch_one(app.state.db.pool())
    .await
    .expect("read the stored event");
    assert_eq!(
        row.get::<Option<i64>, _>("server_cost_usd_micros"),
        None,
        "an unpriced call must not store a cost of zero"
    );
    // No catalog was synced in this test, so that is the reason recorded.
    assert_eq!(
        row.get::<Option<String>, _>("unpriced_reason"),
        Some("no_catalog".to_string())
    );
    // The tokens are still recorded: unpriced volume must stay visible.
    let tokens: i64 = sqlx::query_scalar("SELECT tokens_in FROM telemetry_events WHERE id = ?")
        .bind(event_id.to_string())
        .fetch_one(app.state.db.pool())
        .await
        .expect("read tokens");
    assert_eq!(tokens, 1000);
}

/// Repointing the aggregates must not erase spend that was recorded before
/// Conductor priced anything. A legacy row keeps its client-reported cost,
/// while a freshly priced row reports Conductor's own — and `totals` shows
/// how much of the figure came from each source.
/// Conductor is the only thing that prices a call. A model it has no rate
/// for must surface as unpriced volume, never as zero and never as a figure
/// borrowed from the client — clients report usage and no longer report cost
/// at all, so there is nothing to fall back to and nothing to average in.
#[tokio::test]
async fn a_model_conductor_cannot_price_stays_unpriced() {
    let app = test_app().await;
    let project_id = seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_mixed";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    conductor_server::core::model_pricing::apply_catalog(
        &app.state.db,
        br#"{"anthropic":{"id":"anthropic","models":{
            "claude-sonnet-4":{"id":"claude-sonnet-4","cost":{"input":3,"output":15}}}}}"#,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");
    app.state
        .model_rates
        .reload(&app.state.db)
        .await
        .expect("load rates");

    let (status, body) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            json!({
                "installation_id": installation_id,
                "events": [{
                    "event_id": Uuid::new_v4(),
                    "request_id": Uuid::new_v4(),
                    "event_type": TelemetryEventType::ModelCall,
                    "sequence": 1,
                    "provider": "anthropic",
                    "model": "claude-sonnet-4",
                    "tokens_in": 1_000_000,
                    "tokens_out": 0,
                    "duration_ms": 10,
                    "status": TelemetryEventStatus::Success,
                    "reported_at": chrono::Utc::now().to_rfc3339()
                }, {
                    "event_id": Uuid::new_v4(),
                    "request_id": Uuid::new_v4(),
                    "event_type": TelemetryEventType::ModelCall,
                    "sequence": 1,
                    "provider": "unknown-provider",
                    "model": "unknown-model",
                    "tokens_in": 1_000_000,
                    "tokens_out": 0,
                    "duration_ms": 10,
                    "status": TelemetryEventStatus::Success,
                    "reported_at": chrono::Utc::now().to_rfc3339()
                }]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let totals = app
        .state
        .db
        .resource_usage()
        .analytics(&conductor_storage::repos::ResourceUsageQuery {
            project_id,
            from: chrono::Utc::now() - chrono::Duration::days(1),
            to: chrono::Utc::now() + chrono::Duration::days(1),
            scope: conductor_domain::ResourceUsageScope::All,
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
            limit: 25,
            offset: 0,
        })
        .await
        .expect("analytics")
        .totals;

    assert_eq!(
        totals.estimated_cost_usd_micros, 3_000_000,
        "only the call Conductor could price contributes to the total"
    );
    assert_eq!(
        totals.unpriced_model_calls, 1,
        "the call it could not price must be counted, not costed as zero"
    );
}

/// Cache-write tokens are billed above ordinary input, and they sit inside
/// `tokens_in`. Until EvoFlux reported them separately, Conductor had to
/// price them as input and read systematically low for every model with a
/// cache-write premium. This proves the premium is now applied.
#[tokio::test]
async fn cache_write_tokens_are_priced_at_their_own_rate() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_cachewrite";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    conductor_server::core::model_pricing::apply_catalog(
        &app.state.db,
        br#"{"anthropic":{"id":"anthropic","models":{"claude-sonnet-4":{
            "id":"claude-sonnet-4",
            "cost":{"input":3,"output":15,"cache_read":0.3,"cache_write":3.75}}}}}"#,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");
    app.state
        .model_rates
        .reload(&app.state.db)
        .await
        .expect("load rates");

    let event_id = Uuid::new_v4();
    let (status, body) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            json!({
                "installation_id": installation_id,
                "events": [{
                    "event_id": event_id,
                    "request_id": Uuid::new_v4(),
                    "event_type": TelemetryEventType::ModelCall,
                    "sequence": 1,
                    "provider": "anthropic",
                    "model": "claude-sonnet-4",
                    "tokens_in": 1_000_000,
                    "tokens_out": 0,
                    "cache_read_tokens": 200_000,
                    "cache_write_tokens": 300_000,
                    "reasoning_tokens": 0,
                    "tool_use_tokens": 0,
                    "duration_ms": 10,
                    "status": TelemetryEventStatus::Success,
                    "service_tier": "priority",
                    "reported_at": chrono::Utc::now().to_rfc3339()
                }]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let row = sqlx::query(
        "SELECT server_cost_usd_micros, cache_write_tokens, service_tier \
         FROM telemetry_events WHERE id = ?",
    )
    .bind(event_id.to_string())
    .fetch_one(app.state.db.pool())
    .await
    .expect("read row");

    // input:   500,000 x $3     = $1.50  (both cache kinds carved out)
    // cache_r: 200,000 x $0.30  = $0.06
    // cache_w: 300,000 x $3.75  = $1.125
    let expected = 1_500_000 + 60_000 + 1_125_000;
    assert_eq!(
        row.get::<Option<i64>, _>("server_cost_usd_micros"),
        Some(expected)
    );
    assert_eq!(row.get::<i64, _>("cache_write_tokens"), 300_000);
    assert_eq!(
        row.get::<Option<String>, _>("service_tier"),
        Some("priority".to_string()),
        "the tier is recorded even though it does not change the price yet"
    );
}

/// The whole chain, end to end: EvoFlux names the lane its request actually
/// selected, and Conductor bills that lane's published rate rather than the
/// model's headline one. Without this the lane is stored and ignored, and
/// the most expensive calls in the fleet are the ones priced lowest.
#[tokio::test]
async fn a_lane_with_published_rates_is_billed_at_those_rates() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_lane";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    conductor_server::core::model_pricing::apply_catalog(
        &app.state.db,
        br#"{"openai":{"id":"openai","models":{
            "gpt-5":{"id":"gpt-5","cost":{"input":1.25,"output":10},
              "experimental":{"modes":{
                "priority":{"cost":{"input":2.5,"output":20}}}}}}}}"#,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");
    app.state
        .model_rates
        .reload(&app.state.db)
        .await
        .expect("load rates");

    let ordinary = Uuid::new_v4();
    let lane = Uuid::new_v4();
    let event = |id: Uuid, tier: Option<&str>| {
        let mut value = json!({
            "event_id": id,
            "request_id": Uuid::new_v4(),
            "event_type": TelemetryEventType::ModelCall,
            "sequence": 1,
            "provider": "openai",
            "model": "gpt-5",
            "tokens_in": 1_000_000,
            "tokens_out": 1_000_000,
            "duration_ms": 10,
            "status": TelemetryEventStatus::Success,
            "reported_at": chrono::Utc::now().to_rfc3339()
        });
        if let Some(tier) = tier {
            value["service_tier"] = json!(tier);
        }
        value
    };

    let (status, body) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            json!({
                "installation_id": installation_id,
                "events": [event(ordinary, None), event(lane, Some("priority"))]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    async fn cost_of(app: &TestApp, id: Uuid) -> Option<i64> {
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT server_cost_usd_micros FROM telemetry_events WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_one(app.state.db.pool())
        .await
        .expect("read cost")
    }

    // headline: 1M x $1.25 + 1M x $10
    assert_eq!(cost_of(&app, ordinary).await, Some(11_250_000));
    // priority:  1M x $2.50 + 1M x $20
    assert_eq!(
        cost_of(&app, lane).await,
        Some(22_500_000),
        "the lane publishes its own rates, so it is not billed at the headline price"
    );
}

/// A client that predates these fields must keep working: both are optional
/// on the wire, so omitting them cannot fail a batch.
#[tokio::test]
async fn an_older_client_omitting_the_new_fields_is_still_accepted() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_oldclient";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    // `event_batch` is the pre-existing shape: no cache_write_tokens, no
    // service_tier.
    let (status, body) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            event_batch(&installation_id, Uuid::new_v4()),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["accepted"], 2);
}

/// Ingest must leave a contact record, in the same transaction as the events.
/// Without it, coverage cannot tell a client that reported zero usage from
/// one that never reported, and every project total is a floor of unknown
/// depth.
#[tokio::test]
async fn ingesting_a_batch_records_the_day_as_covered() {
    let app = test_app().await;
    let project_id = seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_coverage";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM installation_contact_days")
        .fetch_one(app.state.db.pool())
        .await
        .expect("count");
    assert_eq!(before, 0, "registering alone is not contact for a day");

    let (status, body) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            event_batch(&installation_id, Uuid::new_v4()),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let row = sqlx::query(
        "SELECT contact_day, telemetry_events, heartbeats, project_id
         FROM installation_contact_days WHERE installation_id = ?",
    )
    .bind(&installation_id)
    .fetch_one(app.state.db.pool())
    .await
    .expect("one contact row for the day");

    assert_eq!(
        row.get::<String, _>("contact_day"),
        chrono::Utc::now().date_naive().to_string(),
        "the day comes from the server clock"
    );
    assert_eq!(row.get::<i64, _>("telemetry_events"), 2);
    assert_eq!(row.get::<i64, _>("heartbeats"), 0);
    assert_eq!(row.get::<String, _>("project_id"), project_id.to_string());
}

/// The member summary reported tokens but no cost, while the doc claimed it
/// did — so a member could not see what their own usage cost. It now reports
/// cost at all three levels, using the same authoritative expression as the
/// project analytics.
#[tokio::test]
async fn member_usage_summary_reports_cost_per_total_model_and_day() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_membercost";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

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
        .expect("load rates");

    let (status, body) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(raw),
            json!({
                "installation_id": installation_id,
                "events": [
                    {
                        "event_id": Uuid::new_v4(),
                        "request_id": Uuid::new_v4(),
                        "event_type": TelemetryEventType::ModelCall,
                        "sequence": 1,
                        "provider": "anthropic",
                        "model": "claude-sonnet-4",
                        "tokens_in": 1_000_000,
                        "tokens_out": 0,
                        "cache_read_tokens": 0,
                        "cache_write_tokens": 0,
                        "reasoning_tokens": 0,
                        "tool_use_tokens": 0,
                        "duration_ms": 10,
                        "status": TelemetryEventStatus::Success,
                        "reported_at": chrono::Utc::now().to_rfc3339()
                    },
                    {
                        "event_id": Uuid::new_v4(),
                        "request_id": Uuid::new_v4(),
                        "event_type": TelemetryEventType::ModelCall,
                        "sequence": 1,
                        "provider": "who-knows",
                        "model": "unlisted",
                        "tokens_in": 500,
                        "tokens_out": 500,
                        "cache_read_tokens": 0,
                        "cache_write_tokens": 0,
                        "reasoning_tokens": 0,
                        "tool_use_tokens": 0,
                        "duration_ms": 10,
                        "status": TelemetryEventStatus::Success,
                        "reported_at": chrono::Utc::now().to_rfc3339()
                    }
                ]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let token = app.token_for(&member).await;
    let (status, summary) = app
        .get(
            &format!("/api/members/{}/usage/summary", member.id),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{summary}");

    assert_eq!(
        summary["estimated_cost_usd_micros"], 3_000_000,
        "one priced call at $3; the unpriced one contributes nothing"
    );
    assert_eq!(
        summary["unpriced_model_calls"], 1,
        "unpriced volume has to stay visible, not read as free"
    );

    let models = summary["models"].as_array().expect("per-model breakdown");
    let sonnet = models
        .iter()
        .find(|entry| entry["model"] == "claude-sonnet-4")
        .expect("sonnet row");
    assert_eq!(sonnet["estimated_cost_usd_micros"], 3_000_000);
    assert_eq!(sonnet["unpriced_calls"], 0);
    let unlisted = models
        .iter()
        .find(|entry| entry["model"] == "unlisted")
        .expect("unlisted row");
    assert_eq!(unlisted["estimated_cost_usd_micros"], 0);
    assert_eq!(unlisted["unpriced_calls"], 1);

    let daily = summary["daily"].as_array().expect("daily series");
    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0]["estimated_cost_usd_micros"], 3_000_000);
}

/// `+00:00` decodes to a space inside a query string, so range parameters use
/// the `Z` spelling of the same instant.
fn rfc3339_z(at: chrono::DateTime<chrono::Utc>) -> String {
    at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Insert a model call with the two timestamps set independently, which ingest
/// never allows: `received_at` is Conductor's own clock.
async fn seed_call_with_timestamps(
    app: &TestApp,
    project_id: Uuid,
    user_id: Uuid,
    request_id: &str,
    reported_at: chrono::DateTime<chrono::Utc>,
    received_at: chrono::DateTime<chrono::Utc>,
    server_cost: i64,
) {
    sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, project_id, user_id, request_id, event_type, sequence,
            provider, model, tokens_in, tokens_out, cache_read_tokens,
            cache_write_tokens, reasoning_tokens, tool_use_tokens, duration_ms,
            status, server_cost_usd_micros, primary_role_snapshot,
            reported_at, received_at
        ) VALUES (?, ?, ?, ?, 'model_call', 1, 'anthropic', 'claude-sonnet-4',
                  1000, 100, 0, 0, 0, 0, 10, 'success', ?, 'user', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(user_id.to_string())
    .bind(request_id)
    .bind(server_cost)
    .bind(reported_at.to_rfc3339())
    .bind(received_at.to_rfc3339())
    .execute(app.state.db.pool())
    .await
    .expect("seed call");
}

/// The member summary windows on `received_at`, matching project analytics and
/// spend limits.
///
/// A client controls `reported_at` and backdates a whole outbox on reconnect,
/// so windowing on it would reopen a period that has already been reported and
/// make a member's page disagree with the admin view of the same range.
#[tokio::test]
async fn member_usage_is_windowed_on_when_conductor_received_it() {
    let app = test_app().await;
    let project_id = seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let now = chrono::Utc::now();
    let long_ago = now - chrono::Duration::days(30);

    // A drained outbox: the work is old, Conductor heard about it today.
    seed_call_with_timestamps(
        &app, project_id, member.id, "drained", long_ago, now, 7_000_000,
    )
    .await;
    // The mirror image, which only a skewed client clock produces. It pins the
    // direction of the filter: reported inside the window, received outside.
    seed_call_with_timestamps(
        &app, project_id, member.id, "skewed", now, long_ago, 99_000_000,
    )
    .await;

    let token = app.token_for(&member).await;
    let (status, summary) = app
        .get(
            &format!(
                "/api/members/{}/usage/summary?from={}&to={}",
                member.id,
                rfc3339_z(now - chrono::Duration::hours(1)),
                rfc3339_z(now + chrono::Duration::hours(1)),
            ),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(
        summary["estimated_cost_usd_micros"], 7_000_000,
        "the drained call counts, the one received outside the window does not"
    );
    assert_eq!(summary["total_requests"], 1);

    let daily = summary["daily"].as_array().expect("daily series");
    assert_eq!(daily.len(), 1, "one bucket, not one per reported day");
    assert_eq!(
        daily[0]["date"],
        now.format("%Y-%m-%d").to_string(),
        "buckets follow the same clock the window does"
    );

    // Activity and tools read the same window, so the page cannot show a
    // request its own KPIs excluded.
    let (status, activity) = app
        .get(
            &format!(
                "/api/members/{}/activity?from={}&to={}",
                member.id,
                rfc3339_z(now - chrono::Duration::hours(1)),
                rfc3339_z(now + chrono::Duration::hours(1)),
            ),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    assert_eq!(activity["total"], 1);
    assert_eq!(activity["items"][0]["request_id"], "drained");
}

/// A very large cost has to survive storage.
///
/// $10,000 in micro-USD is more than four times what a 32-bit column holds —
/// so while the cost columns were declared `INTEGER`, an amount Conductor
/// could price was one Postgres and MySQL could not store. Clients no longer
/// report cost, so the column under test is Conductor's own
/// `server_cost_usd_micros`; the width problem is unchanged.
#[tokio::test]
async fn a_very_large_server_cost_is_stored_and_reported_unchanged() {
    const BIG: i64 = 10_000_000_000;

    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_telemetry_bigcost";
    seed_connection_token(&app, &member, raw).await;
    let installation_id = register(&app, raw).await;

    let batch = event_batch(&installation_id, Uuid::new_v4());
    let (status, body) = app.post("/api/v1/telemetry/batch", Some(raw), batch).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    sqlx::query(
        "UPDATE telemetry_events SET server_cost_usd_micros = ?          WHERE event_type = 'model_call'",
    )
    .bind(BIG)
    .execute(app.state.db.pool())
    .await
    .expect("store a large cost");

    let stored: i64 = sqlx::query_scalar(
        "SELECT server_cost_usd_micros FROM telemetry_events          WHERE server_cost_usd_micros IS NOT NULL",
    )
    .fetch_one(app.state.db.pool())
    .await
    .expect("read the stored cost");
    assert_eq!(stored, BIG, "stored whole, not truncated to 32 bits");

    let token = app.token_for(&member).await;
    let (status, summary) = app
        .get(
            &format!("/api/members/{}/usage/summary", member.id),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["estimated_cost_usd_micros"], BIG);
}
