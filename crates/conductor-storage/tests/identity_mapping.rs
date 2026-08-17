mod support;

use chrono::Utc;
use conductor_domain::{PrimaryRole, SecretScope};
use conductor_storage::{
    InvalidPersistedCredential, InvalidPersistedPrincipal, PersistedCredentialField,
    PersistedPrincipalField, PersistedSecurityReason, StorageError,
};
use support::{connect_test_db, seed_active_user};
use uuid::Uuid;

#[tokio::test]
async fn duplicate_project_identity_is_an_integrity_error_not_an_arbitrary_selection() {
    let db = connect_test_db().await;
    let now = Utc::now().to_rfc3339();
    for project_name in ["first", "second"] {
        sqlx::query(
            "INSERT INTO instance (id, project_name, bind_host, bind_port, setup_completed, jwt_secret, created_at, updated_at) VALUES (?, ?, '127.0.0.1', 4700, 1, 'test-secret', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(project_name)
        .bind(&now)
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("seed duplicate project identity");
    }

    assert!(matches!(
        db.instance().authorization_project_id().await,
        Err(StorageError::Database(_))
    ));
}

#[tokio::test]
async fn unknown_persisted_role_cannot_authenticate() {
    let db = connect_test_db().await;
    let user = seed_active_user(&db, PrimaryRole::User).await;
    sqlx::query("UPDATE users SET primary_role = 'canary-super-admin' WHERE id = ?")
        .bind(user.id.to_string())
        .execute(db.pool())
        .await
        .unwrap();

    assert_principal_error(
        db.users().find_by_id(user.id).await.unwrap_err(),
        Some(user.id),
        PersistedPrincipalField::PrimaryRole,
        PersistedSecurityReason::UnknownValue,
        "canary-super-admin",
    );
}

#[tokio::test]
async fn unknown_persisted_status_cannot_authenticate() {
    let db = connect_test_db().await;
    let user = seed_active_user(&db, PrimaryRole::User).await;
    sqlx::query("UPDATE users SET status = 'canary-enabled' WHERE id = ?")
        .bind(user.id.to_string())
        .execute(db.pool())
        .await
        .unwrap();

    assert_principal_error(
        db.users().find_by_id(user.id).await.unwrap_err(),
        Some(user.id),
        PersistedPrincipalField::Status,
        PersistedSecurityReason::UnknownValue,
        "canary-enabled",
    );
}

#[tokio::test]
async fn corrupt_security_row_returns_invalid_persisted_principal_without_raw_value() {
    let db = connect_test_db().await;
    let user = seed_active_user(&db, PrimaryRole::User).await;

    sqlx::query("UPDATE users SET session_version = -7 WHERE id = ?")
        .bind(user.id.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    assert_principal_error(
        db.users().find_by_id(user.id).await.unwrap_err(),
        Some(user.id),
        PersistedPrincipalField::SessionVersion,
        PersistedSecurityReason::InvalidInteger,
        "-7",
    );

    sqlx::query("UPDATE users SET session_version = 'canary-session-version' WHERE id = ?")
        .bind(user.id.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    assert_principal_error(
        db.users().find_by_id(user.id).await.unwrap_err(),
        Some(user.id),
        PersistedPrincipalField::SessionVersion,
        PersistedSecurityReason::InvalidInteger,
        "canary-session-version",
    );

    sqlx::query(
        "UPDATE users SET session_version = 0, created_at = 'canary-created-at' WHERE id = ?",
    )
    .bind(user.id.to_string())
    .execute(db.pool())
    .await
    .unwrap();
    assert_principal_error(
        db.users().find_by_id(user.id).await.unwrap_err(),
        Some(user.id),
        PersistedPrincipalField::CreatedAt,
        PersistedSecurityReason::InvalidTimestamp,
        "canary-created-at",
    );

    sqlx::query("UPDATE users SET id = 'canary-invalid-user-id' WHERE id = ?")
        .bind(user.id.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    let error = db.users().list().await.unwrap_err();
    assert_principal_error(
        error,
        None,
        PersistedPrincipalField::Id,
        PersistedSecurityReason::InvalidUuid,
        "canary-invalid-user-id",
    );
}

#[tokio::test]
async fn corrupt_secret_row_returns_invalid_persisted_credential_without_raw_value() {
    let db = connect_test_db().await;
    let owner = seed_active_user(&db, PrimaryRole::User).await;
    let hash = "canary-hash-owner";
    let id = insert_secret(
        &db,
        owner.id.to_string(),
        r#"["subscribe_resources"]"#,
        hash,
    )
    .await;

    sqlx::query("UPDATE connection_secrets SET owner_user_id = 'canary-owner-id' WHERE id = ?")
        .bind(id.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    assert_credential_error(
        db.secrets().find_by_hash(hash).await.unwrap_err(),
        Some(id),
        PersistedCredentialField::OwnerUserId,
        PersistedSecurityReason::InvalidUuid,
        "canary-owner-id",
    );

    sqlx::query(
        "UPDATE connection_secrets SET owner_user_id = ?, expires_at = 'canary-expiry' WHERE id = ?",
    )
    .bind(owner.id.to_string())
    .bind(id.to_string())
    .execute(db.pool())
    .await
    .unwrap();
    assert_credential_error(
        db.secrets().find_by_hash(hash).await.unwrap_err(),
        Some(id),
        PersistedCredentialField::ExpiresAt,
        PersistedSecurityReason::InvalidTimestamp,
        "canary-expiry",
    );

    sqlx::query("UPDATE connection_secrets SET id = 'canary-secret-id' WHERE id = ?")
        .bind(id.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    assert_credential_error(
        db.secrets().find_by_hash(hash).await.unwrap_err(),
        None,
        PersistedCredentialField::Id,
        PersistedSecurityReason::InvalidUuid,
        "canary-secret-id",
    );
}

#[tokio::test]
async fn corrupt_revocation_timestamp_is_not_silently_treated_as_valid_or_revoked() {
    let db = connect_test_db().await;
    let owner = seed_active_user(&db, PrimaryRole::User).await;
    let id = insert_secret(
        &db,
        owner.id.to_string(),
        r#"["subscribe_resources"]"#,
        "canary-hash-revoked",
    )
    .await;
    sqlx::query("UPDATE connection_secrets SET revoked_at = 'canary-revoked-at' WHERE id = ?")
        .bind(id.to_string())
        .execute(db.pool())
        .await
        .unwrap();

    assert_credential_error(
        db.secrets().find_by_id(id).await.unwrap_err(),
        Some(id),
        PersistedCredentialField::RevokedAt,
        PersistedSecurityReason::InvalidTimestamp,
        "canary-revoked-at",
    );
}

#[tokio::test]
async fn revoked_credential_remains_available_for_realtime_policy_revalidation() {
    let db = connect_test_db().await;
    let owner = seed_active_user(&db, PrimaryRole::User).await;
    let secret = db
        .secrets()
        .insert(
            owner.id,
            "realtime token",
            "evc_realtime",
            "realtime-hash",
            &[SecretScope::SubscribeResources],
            None,
        )
        .await
        .unwrap();
    assert!(db.secrets().revoke(secret.id, owner.id).await.unwrap());

    let loaded = db.secrets().find_by_id(secret.id).await.unwrap().unwrap();
    assert_eq!(loaded.id, secret.id);
    assert!(loaded.revoked_at.is_some());
    assert!(db
        .secrets()
        .find_by_hash("realtime-hash")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn unknown_stored_scope_never_becomes_empty_or_default_scope_set() {
    let cases = [
        (
            "canary-malformed",
            "not-json",
            PersistedSecurityReason::MalformedPayload,
        ),
        (
            "canary-empty",
            "[]",
            PersistedSecurityReason::EmptyCollection,
        ),
        (
            "canary-duplicate",
            r#"["sync_inventory","sync_inventory"]"#,
            PersistedSecurityReason::DuplicateValue,
        ),
        (
            "canary-unknown",
            r#"["subscribe_resources","canary_root_scope"]"#,
            PersistedSecurityReason::MalformedPayload,
        ),
    ];

    for (hash, scopes, reason) in cases {
        let db = connect_test_db().await;
        let owner = seed_active_user(&db, PrimaryRole::User).await;
        let id = insert_secret(&db, owner.id.to_string(), scopes, hash).await;
        assert_credential_error(
            db.secrets().find_by_hash(hash).await.unwrap_err(),
            Some(id),
            PersistedCredentialField::Scopes,
            reason,
            scopes,
        );
    }
}

#[tokio::test]
async fn database_outage_is_not_reported_as_invalid_persisted_security_data() {
    let db = connect_test_db().await;
    let user = seed_active_user(&db, PrimaryRole::User).await;
    db.pool().close().await;

    assert!(matches!(
        db.users().find_by_id(user.id).await.unwrap_err(),
        StorageError::Database(_)
    ));
    assert!(matches!(
        db.secrets().find_by_hash("unused-hash").await.unwrap_err(),
        StorageError::Database(_)
    ));
}

#[tokio::test]
async fn valid_security_rows_round_trip_without_scope_narrowing() {
    let db = connect_test_db().await;
    let owner = seed_active_user(&db, PrimaryRole::Contribute).await;
    let scopes = SecretScope::ALL;
    let secret = db
        .secrets()
        .insert(
            owner.id,
            "all scopes",
            "evc_prefix",
            "valid-hash",
            &scopes,
            None,
        )
        .await
        .unwrap();
    let loaded = db
        .secrets()
        .find_by_hash("valid-hash")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.id, secret.id);
    assert_eq!(loaded.owner_user_id, owner.id);
    assert_eq!(loaded.scopes, scopes);
}

#[tokio::test]
async fn fresh_schema_rejects_duplicate_connection_token_hashes() {
    let db = connect_test_db().await;
    let first_owner = seed_active_user(&db, PrimaryRole::User).await;
    let second_owner = seed_active_user(&db, PrimaryRole::User).await;
    let hash = "unique-token-hash-canary";

    db.secrets()
        .insert(
            first_owner.id,
            "first token",
            "evc_first",
            hash,
            &[SecretScope::SubscribeResources],
            None,
        )
        .await
        .expect("insert first credential");

    let error = db
        .secrets()
        .insert(
            second_owner.id,
            "second token",
            "evc_second",
            hash,
            &[SecretScope::ReportTelemetry],
            None,
        )
        .await
        .expect_err("the token hash index must reject duplicates");
    assert!(matches!(
        error,
        StorageError::Database(sqlx::Error::Database(ref database))
            if database.is_unique_violation()
    ));
}

#[tokio::test]
async fn duplicate_active_token_hash_never_selects_an_arbitrary_owner_or_scope() {
    let db = connect_test_db().await;
    let first_owner = seed_active_user(&db, PrimaryRole::User).await;
    let second_owner = seed_active_user(&db, PrimaryRole::Admin).await;
    sqlx::query("DROP INDEX idx_connection_secrets_token_hash")
        .execute(db.pool())
        .await
        .expect("simulate a corrupt pre-index database");
    let hash = "duplicate-token-hash-canary";
    insert_secret(
        &db,
        first_owner.id.to_string(),
        r#"["subscribe_resources"]"#,
        hash,
    )
    .await;
    insert_secret(
        &db,
        second_owner.id.to_string(),
        r#"["report_telemetry"]"#,
        hash,
    )
    .await;

    assert_credential_error(
        db.secrets().find_by_hash(hash).await.unwrap_err(),
        None,
        PersistedCredentialField::TokenHash,
        PersistedSecurityReason::DuplicateValue,
        hash,
    );
}

async fn insert_secret(
    db: &conductor_storage::Db,
    owner_id: String,
    scopes: &str,
    hash: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO connection_secrets \
         (id, name, prefix, token_hash, owner_user_id, scopes, created_at) \
         VALUES (?, 'test secret', 'evc_test', ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(hash)
    .bind(owner_id)
    .bind(scopes)
    .bind(Utc::now().to_rfc3339())
    .execute(db.pool())
    .await
    .unwrap();
    id
}

fn assert_principal_error(
    error: StorageError,
    row_id: Option<Uuid>,
    field: PersistedPrincipalField,
    reason: PersistedSecurityReason,
    raw_canary: &str,
) {
    let rendered = error.to_string();
    assert!(!rendered.contains(raw_canary));
    assert!(matches!(
        error,
        StorageError::InvalidPersistedPrincipal(InvalidPersistedPrincipal {
            row_id: actual_id,
            field: actual_field,
            reason: actual_reason,
        }) if actual_id == row_id && actual_field == field && actual_reason == reason
    ));
}

fn assert_credential_error(
    error: StorageError,
    credential_id: Option<Uuid>,
    field: PersistedCredentialField,
    reason: PersistedSecurityReason,
    raw_canary: &str,
) {
    let rendered = error.to_string();
    assert!(!rendered.contains(raw_canary));
    assert!(matches!(
        error,
        StorageError::InvalidPersistedCredential(InvalidPersistedCredential {
            credential_id: actual_id,
            field: actual_field,
            reason: actual_reason,
        }) if actual_id == credential_id && actual_field == field && actual_reason == reason
    ));
}
