mod support;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{PrimaryRole, SecretScope};
use serde_json::{json, Value};
use sqlx::Row;
use support::{test_app, TestApp};
use uuid::Uuid;

const RAW_TOKEN: &str = "evc_role_freshness_same_credential";

#[derive(Debug, PartialEq)]
struct ImmutableSecretSnapshot {
    id: String,
    owner_user_id: String,
    prefix: String,
    token_hash: String,
    scopes: String,
    expires_at: Option<String>,
    revoked_at: Option<String>,
}

#[tokio::test]
async fn compatible_connection_token_survives_role_changes_and_uses_current_role() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let owner = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;
    let secret = app
        .state
        .db
        .secrets()
        .insert(
            owner.id,
            "Role-fresh connection",
            "evc_role",
            &hash_token(RAW_TOKEN),
            &SecretScope::ALL,
            None,
        )
        .await
        .expect("seed connection token");
    let before = secret_snapshot(&app, secret.id).await;
    let installation_key = Uuid::new_v4();

    let first = register(&app, installation_key, "Initial user").await;
    assert_eq!(first["member"]["primary_role"], "user");
    let installation_id = first["installation"]["id"].clone();

    for (role, expected) in [
        (PrimaryRole::Contribute, "contribute"),
        (PrimaryRole::User, "user"),
        (PrimaryRole::Admin, "admin"),
    ] {
        let (status, updated) = app
            .patch(
                &format!("/api/members/{}", owner.id),
                Some(&admin_token),
                json!({"primary_role": role}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        assert_eq!(updated["primary_role"], expected);

        let registered = register(&app, installation_key, expected).await;
        assert_eq!(registered["member"]["primary_role"], expected);
        assert_eq!(registered["installation"]["id"], installation_id);
        exercise_inventory_scope(&app, &installation_id).await;
        exercise_telemetry_scope(&app, &installation_id).await;
        assert_eq!(
            secret_snapshot(&app, secret.id).await,
            before,
            "role change mutated or revoked the existing connection token"
        );
    }

    let secret_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM connection_secrets WHERE owner_user_id = ?")
            .bind(owner.id.to_string())
            .fetch_one(app.state.db.pool())
            .await
            .expect("count owner connection tokens");
    assert_eq!(secret_count, 1, "role changes must not reissue a token");
}

async fn register(app: &TestApp, installation_key: Uuid, label: &str) -> Value {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Idempotency-Key",
        HeaderValue::from_str(&Uuid::new_v4().to_string()).expect("idempotency header"),
    );
    let (status, response) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            headers,
            json!({
                "installation_key": installation_key,
                "display_name": format!("Role freshness {label}"),
                "platform": "linux",
                "evoflux_version": "1.0.0",
                "workspace_association": "role-freshness"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    response
}

async fn exercise_inventory_scope(app: &TestApp, installation_id: &Value) {
    let (status, response) = app
        .put(
            "/api/v1/client/inventory",
            Some(RAW_TOKEN),
            json!({
                "installation_id": installation_id,
                "items": []
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["accepted"], 0);
}

async fn exercise_telemetry_scope(app: &TestApp, installation_id: &Value) {
    let request_id = Uuid::new_v4();
    let (status, response) = app
        .post(
            "/api/v1/telemetry/batch",
            Some(RAW_TOKEN),
            json!({
                "installation_id": installation_id,
                "events": [{
                    "event_id": Uuid::new_v4(),
                    "request_id": format!("role-freshness-{request_id}"),
                    "session_id": null,
                    "event_type": "model_call",
                    "sequence": 1,
                    "agent_name": "role-freshness",
                    "provider": "test-provider",
                    "model": "test-model",
                    "response_model": null,
                    "tokens_in": 1,
                    "tokens_out": 1,
                    "cache_read_tokens": 0,
                    "reasoning_tokens": 0,
                    "tool_use_tokens": 0,
                    "duration_ms": 1,
                    "tool_name": null,
                    "tool_category": null,
                    "status": "success",
                    "error_category": null,
                    "estimated_cost_usd_micros": null,
                    "cost_source": null,
                    "evoflux_version": "1.0.0",
                    "resources": [],
                    "reported_at": chrono::Utc::now()
                }]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["accepted"], 1);
}

async fn secret_snapshot(app: &TestApp, secret_id: Uuid) -> ImmutableSecretSnapshot {
    let row = sqlx::query(
        "SELECT id, owner_user_id, prefix, token_hash, scopes, expires_at, revoked_at \
         FROM connection_secrets WHERE id = ?",
    )
    .bind(secret_id.to_string())
    .fetch_one(app.state.db.pool())
    .await
    .expect("connection secret snapshot");
    ImmutableSecretSnapshot {
        id: row.get("id"),
        owner_user_id: row.get("owner_user_id"),
        prefix: row.get("prefix"),
        token_hash: row.get("token_hash"),
        scopes: row.get("scopes"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
    }
}
