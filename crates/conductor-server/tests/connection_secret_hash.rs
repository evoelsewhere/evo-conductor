mod support;

use axum::http::StatusCode;
use chrono::Utc;
use conductor_auth::hash_token;
use conductor_domain::PrimaryRole;
use support::test_app;
use uuid::Uuid;

const RAW_TOKEN_CANARY: &str = "evc_DUPLICATE_HASH_AUTH_CANARY_never_serialize";
const FIRST_SCOPE_CANARY: &str = "subscribe_resources";
const SECOND_SCOPE_CANARY: &str = "report_telemetry";

#[tokio::test]
async fn duplicate_active_token_hash_fails_closed_before_owner_or_scope_selection() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let first_owner = app.seed_user(PrimaryRole::User).await;
    let second_owner = app.seed_user(PrimaryRole::User).await;

    // Simulate a legacy or externally corrupted database that predates the
    // unique index. Runtime authentication must remain safe even then.
    sqlx::query("DROP INDEX idx_connection_secrets_token_hash")
        .execute(app.state.db.pool())
        .await
        .expect("drop token hash index for corruption canary");

    let token_hash = hash_token(RAW_TOKEN_CANARY);
    let first_id = insert_raw_secret(
        &app,
        first_owner.id,
        "first-owner-canary",
        FIRST_SCOPE_CANARY,
        &token_hash,
    )
    .await;
    let second_id = insert_raw_secret(
        &app,
        second_owner.id,
        "second-owner-canary",
        SECOND_SCOPE_CANARY,
        &token_hash,
    )
    .await;

    let (status, body) = app
        .get("/api/v1/subscribe/resources", Some(RAW_TOKEN_CANARY))
        .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["error"], "internal server error");
    assert_eq!(body["error_code"], "internal_error");
    assert!(body["request_id"].as_str().is_some());

    let rendered = body.to_string();
    for forbidden in [
        RAW_TOKEN_CANARY.to_owned(),
        token_hash,
        first_id.to_string(),
        second_id.to_string(),
        first_owner.id.to_string(),
        second_owner.id.to_string(),
        FIRST_SCOPE_CANARY.to_owned(),
        SECOND_SCOPE_CANARY.to_owned(),
    ] {
        assert!(
            !rendered.contains(&forbidden),
            "safe error leaked duplicate credential state: {forbidden}"
        );
    }

    let marked_used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM connection_secrets WHERE token_hash = ? AND last_used_at IS NOT NULL",
    )
    .bind(hash_token(RAW_TOKEN_CANARY))
    .fetch_one(app.state.db.pool())
    .await
    .expect("read last-used state");
    assert_eq!(
        marked_used, 0,
        "ambiguous credentials must not be marked used"
    );
}

async fn insert_raw_secret(
    app: &support::TestApp,
    owner_id: Uuid,
    name: &str,
    scope: &str,
    token_hash: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    let scopes = serde_json::to_string(&[scope]).expect("serialize scope canary");
    sqlx::query(
        r#"
        INSERT INTO connection_secrets (
            id, name, prefix, token_hash, owner_user_id, scopes, created_at
        ) VALUES (?, ?, 'evc_dupe', ?, ?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(name)
    .bind(token_hash)
    .bind(owner_id.to_string())
    .bind(scopes)
    .bind(Utc::now().to_rfc3339())
    .execute(app.state.db.pool())
    .await
    .expect("insert duplicate token hash canary");
    id
}
