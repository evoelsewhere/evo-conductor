mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use conductor_auth::hash_token;
use conductor_domain::{DecisionReason, PrimaryRole, SecretScope};
use conductor_server::core::authorization::{
    AuthorizationDecisionObserver, AuthorizationEvent, AuthorizationResult,
};
use http_body_util::BodyExt;
use serde_json::Value;
use support::{test_app, TestApp};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Default)]
struct RecordingObserver(std::sync::Mutex<Vec<AuthorizationEvent>>);

impl AuthorizationDecisionObserver for RecordingObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        self.0.lock().expect("observer lock").push(event.clone());
    }
}

async fn seed_secret(
    app: &TestApp,
    owner: &conductor_domain::User,
    raw: &str,
    scopes: &[SecretScope],
) -> Uuid {
    app.state
        .db
        .secrets()
        .insert(
            owner.id,
            "authorization boundary",
            "evc_auth",
            &hash_token(raw),
            scopes,
            None,
        )
        .await
        .expect("seed connection secret")
        .id
}

async fn assert_invalid_connection_principal_does_not_mark_used(
    app: &TestApp,
    raw_token: &str,
    secret_id: Uuid,
) {
    let (status, body) = app
        .get("/api/v1/subscribe/resources", Some(raw_token))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"], "unauthorized");
    assert_eq!(body["code"], 401);
    assert_eq!(body["error_code"], "unauthorized");
    let request_id = body["request_id"]
        .as_str()
        .expect("invalid-principal response request ID");
    Uuid::parse_str(request_id).expect("server-generated request ID");
    assert_eq!(
        body.as_object().map(serde_json::Map::len),
        Some(4),
        "invalid-principal response must expose only the safe error contract"
    );

    let last_used: Option<String> =
        sqlx::query_scalar("SELECT last_used_at FROM connection_secrets WHERE id = ?")
            .bind(secret_id.to_string())
            .fetch_one(app.state.db.pool())
            .await
            .expect("read denied credential last-used metadata");
    assert!(last_used.is_none());
}

#[tokio::test]
async fn dashboard_is_guarded_for_all_fixed_roles() {
    let app = test_app().await;
    app.seed_project_identity().await;
    for (role, expected) in [
        (PrimaryRole::Admin, StatusCode::OK),
        (PrimaryRole::Contribute, StatusCode::OK),
        (PrimaryRole::User, StatusCode::FORBIDDEN),
    ] {
        let token = app.token_for_role(role).await;
        let (status, _) = app.get("/api/dashboard", Some(&token)).await;
        assert_eq!(status, expected, "role {}", role.as_str());
    }
    let (status, _) = app.get("/api/dashboard", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn current_database_role_overrides_the_stale_jwt_claim() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let token = app.token_for(&member).await;

    sqlx::query("UPDATE users SET primary_role = 'contribute' WHERE id = ?")
        .bind(member.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("promote member without changing token");
    let (status, _) = app.get("/api/dashboard", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);

    sqlx::query("UPDATE users SET primary_role = 'user' WHERE id = ?")
        .bind(member.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("demote member without changing token");
    let (status, _) = app.get("/api/dashboard", Some(&token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn browser_and_connection_credentials_cannot_cross_boundaries() {
    let app = test_app().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let browser_token = app.token_for(&member).await;
    let connection_token = "evc_boundary_separation_secret";
    seed_secret(
        &app,
        &member,
        connection_token,
        &[SecretScope::SubscribeResources],
    )
    .await;

    let (status, _) = app.get("/api/dashboard", Some(connection_token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = app
        .get("/api/v1/subscribe/resources", Some(&browser_token))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_scope_is_forbidden_and_does_not_mark_the_secret_used() {
    let app = test_app().await;
    let observer = std::sync::Arc::new(RecordingObserver::default());
    app.state.authorization.set_observer(observer.clone());
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_wrong_scope_boundary_secret";
    let secret_id = seed_secret(&app, &member, raw, &[SecretScope::ReportTelemetry]).await;

    let (status, _) = app.get("/api/v1/subscribe/resources", Some(raw)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let last_used: Option<String> =
        sqlx::query_scalar("SELECT last_used_at FROM connection_secrets WHERE id = ?")
            .bind(secret_id.to_string())
            .fetch_one(app.state.db.pool())
            .await
            .expect("read last-used metadata");
    assert!(last_used.is_none());

    let events = observer.0.lock().expect("observer lock");
    let denial = events.last().expect("wrong-scope decision event");
    assert_eq!(denial.safe_credential_id, Some(secret_id));
    assert_eq!(denial.authorization_result, AuthorizationResult::Denied);
    assert_eq!(denial.reason_code, Some(DecisionReason::DenyScope));
    assert_eq!(denial.required_scope, Some(SecretScope::SubscribeResources));
}

#[tokio::test]
async fn expired_connection_token_is_unauthorized_and_does_not_mark_the_secret_used() {
    let app = test_app().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_expired_authorization_boundary_secret";
    let secret_id = seed_secret(&app, &member, raw, &[SecretScope::SubscribeResources]).await;
    sqlx::query("UPDATE connection_secrets SET expires_at = ? WHERE id = ?")
        .bind((Utc::now() - Duration::minutes(1)).to_rfc3339())
        .bind(secret_id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("expire connection token without using it");

    assert_invalid_connection_principal_does_not_mark_used(&app, raw, secret_id).await;
}

#[tokio::test]
async fn connection_token_with_a_removed_owner_is_unauthorized_and_not_marked_used() {
    let app = test_app().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let raw = "evc_missing_owner_authorization_boundary_secret";
    let secret_id = seed_secret(&app, &member, raw, &[SecretScope::SubscribeResources]).await;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(member.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("remove connection-token owner without revoking the token");

    assert_invalid_connection_principal_does_not_mark_used(&app, raw, secret_id).await;
}

#[tokio::test]
async fn request_id_is_server_generated_and_matches_the_safe_error_body() {
    let app = test_app().await;
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/dashboard")
                .header("X-Request-ID", "caller-controlled")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let header = response
        .headers()
        .get("X-Request-ID")
        .expect("request ID response header")
        .to_str()
        .expect("request ID text")
        .to_owned();
    assert_ne!(header, "caller-controlled");
    Uuid::parse_str(&header).expect("server-generated UUID");

    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect response")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("JSON error response");
    assert_eq!(json["error_code"], "unauthorized");
    assert_eq!(json["request_id"], header);
}

#[tokio::test]
async fn permission_projection_contains_full_fixed_roles_and_current_grants() {
    let app = test_app().await;
    let token = app.token_for_role(PrimaryRole::Contribute).await;

    let (status, body) = app.get("/api/authorization/me", Some(&token)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["schema_version"], 1);
    assert_eq!(body["policy_revision"], "req-004-v1");
    assert_eq!(body["current_role"], "contribute");
    assert_eq!(body["fixed_roles"].as_array().map(Vec::len), Some(3));
    assert!(body["current_grants"]
        .as_array()
        .is_some_and(|grants| !grants.is_empty()));
    assert!(body["permission_metadata"]
        .as_array()
        .is_some_and(|permissions| permissions.len() > 20));
}

#[tokio::test]
async fn corrupt_persisted_principal_fails_closed_as_generic_unauthorized() {
    let app = test_app().await;
    let member = app.seed_user(PrimaryRole::Admin).await;
    let token = app.token_for(&member).await;
    sqlx::query("UPDATE users SET primary_role = 'future_super_admin' WHERE id = ?")
        .bind(member.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("corrupt persisted role");

    let (status, body) = app.get("/api/dashboard", Some(&token)).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "unauthorized");
    assert_eq!(body["error_code"], "unauthorized");
    assert!(!body.to_string().contains("future_super_admin"));
}

#[tokio::test]
async fn operational_storage_failure_remains_safe_internal_error() {
    let app = test_app().await;
    let token = app.token_for_role(PrimaryRole::Admin).await;
    app.state.db.pool().close().await;

    let (status, body) = app.get("/api/dashboard", Some(&token)).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "internal server error");
    assert_eq!(body["error_code"], "internal_error");
    assert!(body["request_id"].as_str().is_some());
}
