mod support;

use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{
    AuthorizationAction, ClientPlatform, DecisionReason, PrimaryRole, SecretScope,
    TelemetryBatchRequest, TelemetryEventStatus, TelemetryEventType, TelemetryToolCategory, User,
};
use conductor_server::core::authorization::{
    AuthorizationDecisionObserver, AuthorizationEvent, AuthorizationResult, AuthorizationService,
    AuthorizationStage,
};
use serde_json::{json, Value};
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

#[tokio::test]
async fn inventory_rejects_unknown_state_and_cross_resource_versions() {
    let app = test_app().await;
    let project_id = seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let owner = app.seed_user(PrimaryRole::Contribute).await;
    let (first_resource_id, first_version_id) =
        seed_resource(&app, project_id, owner.id, "agent", "first", "First").await;
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

#[tokio::test]
async fn resource_usage_analytics_attributes_member_role_version_tokens_and_cost() {
    let observer = Arc::new(RecordingObserver::default());
    let app = test_app_with_authorization(AuthorizationService::new(observer.clone())).await;
    let project_id = seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let resource_owner = app.seed_user(PrimaryRole::Contribute).await;
    let (resource_id, version_id) = seed_resource(
        &app,
        project_id,
        resource_owner.id,
        "agent",
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
    batch["events"][0]["estimated_cost_usd_micros"] = json!(1250);
    batch["events"][0]["cost_source"] = json!("evoflux_catalog");
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
        "tool_name": null,
        "tool_category": null,
        "status": "success",
        "error_category": null,
        "resources": [agent_reference, skill_reference],
        "reported_at": chrono::Utc::now().to_rfc3339()
    }));
    serde_json::from_value::<TelemetryBatchRequest>(batch.clone()).expect("valid telemetry batch");

    let (status, response) = app.post("/api/v1/telemetry/batch", Some(raw), batch).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["accepted"], 3);

    let admin_token = app.token_for_role(PrimaryRole::Admin).await;
    let (status, analytics) = app
        .get("/api/analytics/resource-usage", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{analytics}");
    assert_eq!(analytics["totals"]["requests"], 1);
    assert_eq!(analytics["totals"]["resource_uses"], 2);
    assert_eq!(analytics["totals"]["model_calls"], 1);
    assert_eq!(analytics["totals"]["cache_read_tokens"], 20);
    assert_eq!(analytics["totals"]["reasoning_tokens"], 10);
    assert_eq!(analytics["totals"]["total_tokens"], 180);
    assert_eq!(analytics["totals"]["average_tokens_per_request"], 180);
    assert_eq!(analytics["totals"]["estimated_cost_usd_micros"], 1250);
    assert_eq!(analytics["totals"]["reported_installations"], 2);
    assert_eq!(analytics["totals"]["installed_installations"], 1);
    assert_eq!(analytics["totals"]["installed_members"], 1);
    assert_eq!(analytics["roles"][0]["primary_role"], "user");
    assert_eq!(analytics["roles"][0]["tool_calls"], 1);
    assert_eq!(analytics["tools"][0]["tool_name"], "read_file");
    assert_eq!(analytics["tools"][0]["calls"], 1);
    let resource_rows = analytics["resources"].as_array().expect("resource rows");
    assert_eq!(resource_rows.len(), 2);
    assert!(resource_rows
        .iter()
        .any(|row| row["resource_id"] == resource_id.to_string()));
    assert_eq!(analytics["members"][0]["primary_role"], "user");
    assert_eq!(
        analytics["activity"][0]["display_name"],
        member.display_name
    );

    let contributor_token = app.token_for(&resource_owner).await;
    let (status, contributor_analytics) = app
        .get("/api/analytics/resource-usage", Some(&contributor_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{contributor_analytics}");
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
    assert_eq!(filtered["totals"]["requests"], 1);
    assert_eq!(filtered["totals"]["successes"], 1);
    assert_eq!(filtered["totals"]["model_calls"], 1);
    assert_eq!(filtered["totals"]["total_tokens"], 180);
    assert_eq!(filtered["activity"][0]["total_tokens"], 180);
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
    assert_eq!(detail["request"]["estimated_cost_usd_micros"], 1250);
    let model_event = detail["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| event["event_type"] == "model_call")
        .expect("model event");
    assert_eq!(model_event["resources"].as_array().map(Vec::len), Some(2));

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
