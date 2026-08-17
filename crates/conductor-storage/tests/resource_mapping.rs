mod support;

use chrono::Utc;
use conductor_domain::{
    CreateResourceRequest, PrimaryRole, ResourceKind, ResourceVisibility, SetupRequest,
};
use conductor_storage::repos::DraftContent;
use conductor_storage::{
    Db, InvalidPersistedResource, PersistedResourceField, PersistedSecurityReason, StorageError,
};
use support::{connect_test_db, PLACEHOLDER_PASSWORD_HASH};
use uuid::Uuid;

#[tokio::test]
async fn browser_resource_list_rejects_corrupt_policy_fields_instead_of_dropping_rows() {
    let cases = [
        (
            "id",
            "canary-invalid-resource-id",
            None,
            PersistedResourceField::Id,
            PersistedSecurityReason::InvalidUuid,
        ),
        (
            "project_id",
            "canary-invalid-project-id",
            Some(()),
            PersistedResourceField::ProjectId,
            PersistedSecurityReason::InvalidUuid,
        ),
        (
            "kind",
            "canary-root-resource",
            Some(()),
            PersistedResourceField::Kind,
            PersistedSecurityReason::UnknownValue,
        ),
        (
            "status",
            "canary-always-active",
            Some(()),
            PersistedResourceField::Status,
            PersistedSecurityReason::UnknownValue,
        ),
        (
            "owner_user_id",
            "canary-invalid-owner-id",
            Some(()),
            PersistedResourceField::OwnerUserId,
            PersistedSecurityReason::InvalidUuid,
        ),
        (
            "visibility",
            "canary-public",
            Some(()),
            PersistedResourceField::Visibility,
            PersistedSecurityReason::UnknownValue,
        ),
        (
            "payload",
            "canary-not-json",
            Some(()),
            PersistedResourceField::Payload,
            PersistedSecurityReason::MalformedPayload,
        ),
    ];

    for (column, raw_canary, has_resource_id, field, reason) in cases {
        let (db, _project_id, admin_id, resource_id) = seeded_resource().await;
        corrupt_resource_column(&db, resource_id, column, raw_canary).await;

        let error = db
            .resources()
            .list_for_actor(admin_id, PrimaryRole::Admin)
            .await
            .expect_err("a corrupt resource row must fail the complete list");
        assert_resource_error(
            error,
            has_resource_id.map(|()| resource_id),
            field,
            reason,
            raw_canary,
        );
    }
}

#[tokio::test]
async fn visible_resource_ids_rejects_an_invalid_uuid_instead_of_narrowing_the_audience() {
    let (db, _project_id, admin_id, resource_id) = seeded_resource().await;
    sqlx::query("UPDATE resources SET status = 'published' WHERE id = ?")
        .bind(resource_id.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    let raw_canary = "canary-visible-resource-id";
    corrupt_resource_column(&db, resource_id, "id", raw_canary).await;

    assert_resource_error(
        db.resources()
            .visible_resource_ids(admin_id)
            .await
            .expect_err("corrupt visible IDs must not be filtered out"),
        None,
        PersistedResourceField::Id,
        PersistedSecurityReason::InvalidUuid,
        raw_canary,
    );
}

#[tokio::test]
async fn effective_version_delivery_rejects_corrupt_identity_channel_and_payload() {
    let cases = [
        EffectiveCase {
            version_id: Some("canary-invalid-version-id"),
            payload: r#"{"portable":true}"#,
            release_channel: Some("published"),
            content_size: 1,
            field: PersistedResourceField::VersionId,
            reason: PersistedSecurityReason::InvalidUuid,
            through_list: false,
            raw_canary: "canary-invalid-version-id",
        },
        EffectiveCase {
            version_id: None,
            payload: "canary-malformed-version-payload",
            release_channel: Some("published"),
            content_size: 1,
            field: PersistedResourceField::Payload,
            reason: PersistedSecurityReason::MalformedPayload,
            through_list: true,
            raw_canary: "canary-malformed-version-payload",
        },
        EffectiveCase {
            version_id: None,
            payload: r#"{"portable":true}"#,
            release_channel: Some("canary-release-channel"),
            content_size: 1,
            field: PersistedResourceField::ReleaseChannel,
            reason: PersistedSecurityReason::UnknownValue,
            through_list: false,
            raw_canary: "canary-release-channel",
        },
        EffectiveCase {
            version_id: None,
            payload: r#"{"portable":true}"#,
            release_channel: Some("published"),
            content_size: -1,
            field: PersistedResourceField::ContentSize,
            reason: PersistedSecurityReason::InvalidInteger,
            through_list: true,
            raw_canary: "-1",
        },
    ];

    for case in cases {
        let (db, project_id, admin_id, resource_id) = seeded_resource().await;
        let version_id = case
            .version_id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        insert_effective_version(
            &db,
            project_id,
            admin_id,
            resource_id,
            &version_id,
            case.payload,
            case.release_channel,
            case.content_size,
        )
        .await;

        let error = if case.through_list {
            db.resources()
                .list_effective_versions(admin_id)
                .await
                .map(|_| ())
                .expect_err("the effective-version list must reject corrupt delivery state")
        } else {
            db.resources()
                .effective_version(resource_id, admin_id)
                .await
                .map(|_| ())
                .expect_err("single effective-version delivery must reject corrupt state")
        };
        assert_resource_error(
            error,
            Some(resource_id),
            case.field,
            case.reason,
            case.raw_canary,
        );
    }
}

#[tokio::test]
async fn corrupt_effective_version_history_is_not_silently_dropped() {
    let (db, project_id, admin_id, resource_id) = seeded_resource().await;
    let current_version_id = Uuid::new_v4().to_string();
    insert_effective_version(
        &db,
        project_id,
        admin_id,
        resource_id,
        &current_version_id,
        r#"{"portable":true}"#,
        Some("published"),
        1,
    )
    .await;
    sqlx::query("UPDATE resource_versions SET version = '1.0.1' WHERE id = ?")
        .bind(&current_version_id)
        .execute(db.pool())
        .await
        .unwrap();

    let raw_canary = "canary-history-version-id";
    insert_version_row(
        &db,
        project_id,
        admin_id,
        resource_id,
        raw_canary,
        r#"{"portable":true}"#,
        Some("published"),
        1,
        "deprecated",
    )
    .await;

    assert_resource_error(
        db.resources()
            .effective_version(resource_id, admin_id)
            .await
            .expect_err("corrupt history must fail the effective version as a whole"),
        Some(resource_id),
        PersistedResourceField::VersionId,
        PersistedSecurityReason::InvalidUuid,
        raw_canary,
    );
}

#[tokio::test]
async fn resource_database_outage_remains_an_operational_error() {
    let (db, _project_id, admin_id, resource_id) = seeded_resource().await;
    db.pool().close().await;

    assert!(matches!(
        db.resources().list_all().await.unwrap_err(),
        StorageError::Database(_)
    ));
    assert!(matches!(
        db.resources()
            .effective_version(resource_id, admin_id)
            .await
            .unwrap_err(),
        StorageError::Database(_)
    ));
}

#[derive(Clone, Copy)]
struct EffectiveCase {
    version_id: Option<&'static str>,
    payload: &'static str,
    release_channel: Option<&'static str>,
    content_size: i64,
    field: PersistedResourceField,
    reason: PersistedSecurityReason,
    through_list: bool,
    raw_canary: &'static str,
}

async fn seeded_resource() -> (Db, Uuid, Uuid, Uuid) {
    let db = connect_test_db().await;
    let (project, admin) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: format!("resource-mapping-{}", Uuid::new_v4().simple()),
                display_name: Some("Resource mapping test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: format!("resource-{}@example.test", Uuid::new_v4().simple()),
                admin_display_name: "Resource mapping admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            PLACEHOLDER_PASSWORD_HASH,
            "resource-mapping-jwt-secret",
            None,
        )
        .await
        .expect("complete setup");
    let digest = "a".repeat(64);
    let resource = db
        .resources()
        .create(
            project.id,
            &CreateResourceRequest {
                kind: ResourceKind::Skill,
                slug: format!("mapping-{}", Uuid::new_v4().simple()),
                name: "Mapping canary".into(),
                description: None,
                version: "0.1.0".into(),
                visibility: ResourceVisibility::Shared,
                payload: serde_json::json!({}),
                changelog: None,
            },
            admin.id,
            &DraftContent {
                artifact_key: format!("sha256/aa/{digest}"),
                sha256: digest,
                size: 1,
                metadata_payload: serde_json::json!({"portable": true}),
            },
        )
        .await
        .expect("create resource");
    (db, project.id, admin.id, resource.id)
}

async fn corrupt_resource_column(db: &Db, resource_id: Uuid, column: &str, value: &str) {
    let mut connection = db.pool().acquire().await.unwrap();
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .unwrap();
    let query = format!("UPDATE resources SET {column} = ? WHERE id = ?");
    sqlx::query(&query)
        .bind(value)
        .bind(resource_id.to_string())
        .execute(&mut *connection)
        .await
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
async fn insert_effective_version(
    db: &Db,
    project_id: Uuid,
    admin_id: Uuid,
    resource_id: Uuid,
    version_id: &str,
    payload: &str,
    release_channel: Option<&str>,
    content_size: i64,
) {
    sqlx::query("UPDATE resources SET status = 'published' WHERE id = ?")
        .bind(resource_id.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    insert_version_row(
        db,
        project_id,
        admin_id,
        resource_id,
        version_id,
        payload,
        release_channel,
        content_size,
        "published",
    )
    .await;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO resource_release_channels (
            project_id, resource_id, channel, version_id, updated_by, updated_at
        ) VALUES (?, ?, 'published', ?, ?, ?)
        "#,
    )
    .bind(project_id.to_string())
    .bind(resource_id.to_string())
    .bind(version_id)
    .bind(admin_id.to_string())
    .bind(now)
    .execute(db.pool())
    .await
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
async fn insert_version_row(
    db: &Db,
    project_id: Uuid,
    admin_id: Uuid,
    resource_id: Uuid,
    version_id: &str,
    payload: &str,
    release_channel: Option<&str>,
    content_size: i64,
    status: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO resource_versions (
            id, project_id, resource_id, version, status, payload, release_channel,
            content_sha256, content_size, created_by, created_at
        ) VALUES (?, ?, ?, '1.0.0', ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(version_id)
    .bind(project_id.to_string())
    .bind(resource_id.to_string())
    .bind(status)
    .bind(payload)
    .bind(release_channel)
    .bind("a".repeat(64))
    .bind(content_size)
    .bind(admin_id.to_string())
    .bind(Utc::now().to_rfc3339())
    .execute(db.pool())
    .await
    .unwrap();
}

fn assert_resource_error(
    error: StorageError,
    resource_id: Option<Uuid>,
    field: PersistedResourceField,
    reason: PersistedSecurityReason,
    raw_canary: &str,
) {
    let rendered = error.to_string();
    assert!(!rendered.contains(raw_canary));
    assert!(matches!(
        error,
        StorageError::InvalidPersistedResource(InvalidPersistedResource {
            resource_id: actual_resource_id,
            field: actual_field,
            reason: actual_reason,
        }) if actual_resource_id == resource_id && actual_field == field && actual_reason == reason
    ));
}
