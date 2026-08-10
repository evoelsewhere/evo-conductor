mod support;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{PrimaryRole, SecretScope, User, UserStatus};
use serde_json::{json, Value};
use support::{test_app, TestApp};
use uuid::Uuid;

const RAW_TOKEN: &str = "evc_registration_test_secret";

async fn seed_instance(app: &TestApp) -> Uuid {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, display_name, bind_host, bind_port, public_url, logo_url,
            collection_level, setup_completed, jwt_secret, created_at, updated_at
        ) VALUES (?, 'Evo Project', 'Evo', '127.0.0.1', 4700,
                  'http://127.0.0.1:4700', 'https://example.test/logo.svg',
                  'L1', 1, 'unused-test-jwt', ?, ?)
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

async fn seed_connection_token(
    app: &TestApp,
    user: &User,
    raw: &str,
    scopes: &[SecretScope],
) -> Uuid {
    app.state
        .db
        .secrets()
        .insert(
            user.id,
            "EvoFlux",
            "evc_regi",
            &hash_token(raw),
            scopes,
            None,
        )
        .await
        .expect("seed connection token")
        .id
}

fn registration_body(key: Uuid, name: &str) -> Value {
    json!({
        "installation_key": key,
        "display_name": name,
        "platform": "macos",
        "evoflux_version": "0.8.0",
        "workspace_association": "Marketing site"
    })
}

fn idempotency_headers(key: Uuid) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Idempotency-Key",
        HeaderValue::from_str(&key.to_string()).expect("header value"),
    );
    headers
}

#[tokio::test]
async fn registration_is_idempotent_and_returns_server_owned_bootstrap() {
    let app = test_app().await;
    let _instance_id = seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    seed_connection_token(&app, &member, RAW_TOKEN, &[SecretScope::SubscribeResources]).await;
    let local_key = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();

    let (status, first) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(idempotency_key),
            registration_body(local_key, "EvoFlux on macOS"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["project"]["name"], "Evo Project");
    assert_eq!(first["project"]["display_name"], "Evo");
    assert_eq!(first["member"]["id"], member.id.to_string());
    assert_eq!(first["member"]["primary_role"], "user");
    assert_eq!(first["policy"]["collection_level"], "L1");
    assert_eq!(first["policy"]["telemetry"]["enabled"], true);
    assert_eq!(first["installation"]["heartbeat_interval_seconds"], 60);

    let (status, replay) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(idempotency_key),
            registration_body(local_key, "EvoFlux on macOS"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["installation"]["id"], first["installation"]["id"]);

    let (status, updated) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(Uuid::new_v4()),
            registration_body(local_key, "Renamed installation"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["installation"]["id"], first["installation"]["id"]);
    assert_eq!(
        updated["installation"]["display_name"],
        "Renamed installation"
    );

    sqlx::query(
        "UPDATE client_registration_idempotency SET created_at = '2000-01-01T00:00:00Z' \
         WHERE idempotency_key = ?",
    )
    .bind(idempotency_key.to_string())
    .execute(app.state.db.pool())
    .await
    .expect("age replay record");
    let (status, expired_replay) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(idempotency_key),
            registration_body(local_key, "Expired replay key"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{expired_replay}");
    assert_eq!(
        expired_replay["installation"]["id"],
        first["installation"]["id"]
    );
    assert_eq!(
        app.state
            .db
            .client_installations()
            .list_for_user(member.id)
            .await
            .expect("list installations")
            .len(),
        1
    );
}

#[tokio::test]
async fn registration_rejects_conflicting_replay_and_wrong_scope() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    seed_connection_token(&app, &member, RAW_TOKEN, &[SecretScope::SubscribeResources]).await;
    let local_key = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();
    let (status, _) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(idempotency_key),
            registration_body(local_key, "First"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(idempotency_key),
            registration_body(local_key, "Changed"),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let other = app.seed_user(PrimaryRole::User).await;
    let other_token = "evc_other_registration_owner";
    seed_connection_token(
        &app,
        &other,
        other_token,
        &[SecretScope::SubscribeResources],
    )
    .await;
    let (status, body) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(other_token),
            idempotency_headers(Uuid::new_v4()),
            registration_body(local_key, "Stolen installation key"),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let wrong_scope = "evc_wrong_scope";
    seed_connection_token(&app, &member, wrong_scope, &[SecretScope::ReportTelemetry]).await;
    let (status, _) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(wrong_scope),
            idempotency_headers(Uuid::new_v4()),
            registration_body(Uuid::new_v4(), "Wrong scope"),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn registration_rejects_missing_idempotency_invalid_labels_and_unknown_tokens() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    seed_connection_token(&app, &member, RAW_TOKEN, &[SecretScope::SubscribeResources]).await;

    let (status, _) = app
        .post(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            registration_body(Uuid::new_v4(), "No idempotency key"),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut unsafe_workspace = registration_body(Uuid::new_v4(), "Desktop");
    unsafe_workspace["workspace_association"] = json!("/Users/member/private-project");
    let (status, _) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(Uuid::new_v4()),
            unsafe_workspace,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some("evc_unknown"),
            idempotency_headers(Uuid::new_v4()),
            registration_body(Uuid::new_v4(), "Unknown token"),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn heartbeat_is_owner_scoped_and_revocation_stops_access() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let secret_id =
        seed_connection_token(&app, &member, RAW_TOKEN, &[SecretScope::SubscribeResources]).await;
    let (status, registered) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(Uuid::new_v4()),
            registration_body(Uuid::new_v4(), "Primary"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{registered}");
    let installation_id = registered["installation"]["id"]
        .as_str()
        .expect("installation id");

    let (status, heartbeat) = app
        .post(
            "/api/v1/client/heartbeat",
            Some(RAW_TOKEN),
            json!({"installation_id": installation_id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{heartbeat}");
    assert_eq!(heartbeat["connection_state"], "active");

    let other = app.seed_user(PrimaryRole::User).await;
    let other_token = "evc_other_member";
    seed_connection_token(
        &app,
        &other,
        other_token,
        &[SecretScope::SubscribeResources],
    )
    .await;
    let (status, _) = app
        .post(
            "/api/v1/client/heartbeat",
            Some(other_token),
            json!({"installation_id": installation_id}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    assert!(app
        .state
        .db
        .secrets()
        .revoke(secret_id, member.id)
        .await
        .expect("revoke"));
    let (status, _) = app
        .post(
            "/api/v1/client/heartbeat",
            Some(RAW_TOKEN),
            json!({"installation_id": installation_id}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn member_installation_list_is_privacy_safe_and_authorized() {
    let app = test_app().await;
    seed_instance(&app).await;
    let member = app.seed_user(PrimaryRole::User).await;
    seed_connection_token(&app, &member, RAW_TOKEN, &[SecretScope::SubscribeResources]).await;
    for name in ["Mac", "PC"] {
        let (status, _) = app
            .post_with_headers(
                "/api/v1/client/register",
                Some(RAW_TOKEN),
                idempotency_headers(Uuid::new_v4()),
                registration_body(Uuid::new_v4(), name),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
    }

    let self_token = app.token_for(&member).await;
    let (status, rows) = app
        .get(
            &format!("/api/members/{}/installations", member.id),
            Some(&self_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rows}");
    assert_eq!(rows.as_array().expect("rows").len(), 2);
    assert!(rows[0].get("workspace_association").is_none());
    assert!(rows[0].get("installation_key").is_none());

    let stranger = app.seed_user(PrimaryRole::User).await;
    let stranger_token = app.token_for(&stranger).await;
    let (status, _) = app
        .get(
            &format!("/api/members/{}/installations", member.id),
            Some(&stranger_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let admin = app.seed_user(PrimaryRole::Admin).await;
    let admin_token = app.token_for(&admin).await;
    let (status, rows) = app
        .get(
            &format!("/api/members/{}/installations", member.id),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rows}");
    assert_eq!(rows.as_array().expect("rows").len(), 2);

    app.state
        .db
        .users()
        .set_status(member.id, UserStatus::Disabled)
        .await
        .expect("disable member");
    let (status, _) = app
        .post(
            "/api/v1/client/heartbeat",
            Some(RAW_TOKEN),
            json!({"installation_id": Uuid::new_v4()}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
