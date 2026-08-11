mod support;

use std::collections::HashMap;

use conductor_domain::{
    CreateResourceRequest, PrimaryRole, ReleaseChannel, ReleaseResourceRequest, ResourceKind,
    ResourceVisibility, SetupRequest, VersionMode,
};
use conductor_storage::repos::{ReleaseContent, ResourceVersionLifecycleError};
use sqlx::Row;
use support::{connect_test_db, seed_active_user, PLACEHOLDER_PASSWORD_HASH};

#[tokio::test]
async fn beta_release_changes_are_scoped_to_current_and_removed_beta_members() {
    let db = connect_test_db().await;
    let (project, admin) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "beta-feed-test".into(),
                display_name: Some("Beta feed test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "admin@example.test".into(),
                admin_display_name: "Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            PLACEHOLDER_PASSWORD_HASH,
            "jwt-test-secret",
            None,
        )
        .await
        .expect("complete setup");
    let first_member = seed_active_user(&db, PrimaryRole::User).await;
    let second_member = seed_active_user(&db, PrimaryRole::User).await;
    let resource = db
        .resources()
        .create(
            project.id,
            &CreateResourceRequest {
                kind: ResourceKind::Skill,
                slug: "managed-skill".into(),
                name: "Managed Skill".into(),
                description: None,
                version: "0.1.0".into(),
                visibility: ResourceVisibility::Shared,
                payload: serde_json::json!({}),
                changelog: None,
            },
            admin.id,
        )
        .await
        .expect("create resource");
    let content = ReleaseContent {
        sha256: "a".repeat(64),
        size: 2,
        artifact_key: None,
        updated_payload: None,
    };

    db.resources()
        .release(
            resource.id,
            &ReleaseResourceRequest {
                channel: ReleaseChannel::Beta,
                version_mode: VersionMode::Auto,
                manual_version: None,
                draft_revision: 0,
                changelog: None,
                beta_member_ids: vec![first_member.id],
                minimum_evoflux_version: None,
            },
            &content,
            admin.id,
        )
        .await
        .expect("first beta release");
    db.resources()
        .release(
            resource.id,
            &ReleaseResourceRequest {
                channel: ReleaseChannel::Beta,
                version_mode: VersionMode::Auto,
                manual_version: None,
                draft_revision: 0,
                changelog: None,
                beta_member_ids: vec![second_member.id],
                minimum_evoflux_version: None,
            },
            &content,
            admin.id,
        )
        .await
        .expect("second beta release");

    let rows = sqlx::query(
        "SELECT effective_user_id FROM resource_changes WHERE resource_id = ? ORDER BY sequence",
    )
    .bind(resource.id.to_string())
    .fetch_all(db.pool())
    .await
    .expect("load change audience");
    let mut counts = HashMap::<String, usize>::new();
    for row in rows {
        let user_id = row
            .get::<Option<String>, _>("effective_user_id")
            .expect("beta changes must never be global");
        *counts.entry(user_id).or_default() += 1;
    }
    assert_eq!(counts.get(&first_member.id.to_string()), Some(&2));
    assert_eq!(counts.get(&second_member.id.to_string()), Some(&1));
}

#[tokio::test]
async fn deprecated_versions_remain_auditable_and_require_confirmation_to_restore() {
    let db = connect_test_db().await;
    let (project, admin) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "version-lifecycle-test".into(),
                display_name: Some("Version lifecycle test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "lifecycle-admin@example.test".into(),
                admin_display_name: "Lifecycle Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            PLACEHOLDER_PASSWORD_HASH,
            "lifecycle-jwt-test-secret",
            None,
        )
        .await
        .expect("complete setup");
    let first_payload = serde_json::json!({
        "files": [{"path": "SKILL.md", "content": "first release"}]
    });
    let second_payload = serde_json::json!({
        "files": [{"path": "SKILL.md", "content": "second release"}]
    });
    let resource = db
        .resources()
        .create(
            project.id,
            &CreateResourceRequest {
                kind: ResourceKind::Skill,
                slug: "lifecycle-skill".into(),
                name: "Lifecycle Skill".into(),
                description: None,
                version: "0.1.0".into(),
                visibility: ResourceVisibility::Shared,
                payload: first_payload.clone(),
                changelog: None,
            },
            admin.id,
        )
        .await
        .expect("create resource");

    let first = db
        .resources()
        .release(
            resource.id,
            &ReleaseResourceRequest {
                channel: ReleaseChannel::Published,
                version_mode: VersionMode::Auto,
                manual_version: None,
                draft_revision: 0,
                changelog: Some("first".into()),
                beta_member_ids: vec![],
                minimum_evoflux_version: None,
            },
            &ReleaseContent {
                sha256: "a".repeat(64),
                size: 13,
                artifact_key: None,
                updated_payload: Some(first_payload.to_string()),
            },
            admin.id,
        )
        .await
        .expect("release first version");
    let second = db
        .resources()
        .release(
            resource.id,
            &ReleaseResourceRequest {
                channel: ReleaseChannel::Published,
                version_mode: VersionMode::Auto,
                manual_version: None,
                draft_revision: 1,
                changelog: Some("second".into()),
                beta_member_ids: vec![],
                minimum_evoflux_version: None,
            },
            &ReleaseContent {
                sha256: "b".repeat(64),
                size: 14,
                artifact_key: None,
                updated_payload: Some(second_payload.to_string()),
            },
            admin.id,
        )
        .await
        .expect("release second version");

    let active_error = db
        .resources()
        .deprecate_version(resource.id, second.version_id, admin.id, "active version")
        .await
        .expect_err("active release must not be deprecated");
    assert!(matches!(
        active_error,
        ResourceVersionLifecycleError::ActiveRelease
    ));

    let deprecated = db
        .resources()
        .deprecate_version(
            resource.id,
            first.version_id,
            admin.id,
            "Known compatibility issue",
        )
        .await
        .expect("deprecate historical version");
    assert_eq!(deprecated.status.as_str(), "deprecated");
    assert_eq!(
        deprecated.deprecation_reason.as_deref(),
        Some("Known compatibility issue")
    );
    assert_eq!(deprecated.deprecated_by, Some(admin.id));
    assert!(deprecated.deprecated_at.is_some());
    assert!(deprecated.active_channel.is_none());
    assert_eq!(deprecated.content_sha256, "a".repeat(64));

    let effective = db
        .resources()
        .effective_version(resource.id, admin.id)
        .await
        .expect("load effective version metadata")
        .expect("published version remains effective");
    assert_eq!(effective.version_id, second.version_id);
    assert_eq!(effective.changelog.as_deref(), Some("second"));
    assert_eq!(effective.version_history.len(), 2);
    let deprecated_notice = effective
        .version_history
        .iter()
        .find(|notice| notice.version_id == first.version_id)
        .expect("deprecated installed version remains visible in safe history");
    assert_eq!(deprecated_notice.status.as_str(), "deprecated");
    assert_eq!(
        deprecated_notice.deprecation_reason.as_deref(),
        Some("Known compatibility issue")
    );

    let deprecation_changes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM resource_changes WHERE resource_id = ? AND change_kind = 'deprecate'",
    )
    .bind(resource.id.to_string())
    .fetch_one(db.pool())
    .await
    .expect("count deprecation change notifications");
    assert_eq!(deprecation_changes, 1);

    let confirmation_error = db
        .resources()
        .restore_version_to_draft(resource.id, first.version_id, admin.id, 2, false)
        .await
        .expect_err("deprecated restore must require confirmation");
    assert!(matches!(
        confirmation_error,
        ResourceVersionLifecycleError::DeprecatedConfirmationRequired
    ));

    let restored = db
        .resources()
        .restore_version_to_draft(resource.id, first.version_id, admin.id, 2, true)
        .await
        .expect("restore deprecated source after confirmation");
    assert_eq!(restored.revision, 3);
    assert_eq!(restored.files[0].content, "first release");

    let current = db
        .resources()
        .find_by_id(resource.id)
        .await
        .expect("load resource")
        .expect("resource exists");
    assert_eq!(current.highest_version.as_deref(), Some("0.1.1"));
    assert_eq!(current.version, "0.1.1");
    assert_eq!(current.payload, first_payload);
    let versions = db
        .resources()
        .versions(resource.id)
        .await
        .expect("load immutable history");
    assert_eq!(versions.len(), 2);
    assert_eq!(
        versions
            .iter()
            .find(|version| version.id == second.version_id)
            .and_then(|version| version.active_channel),
        Some(ReleaseChannel::Published)
    );

    let events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM resource_version_events WHERE resource_id = ?")
            .bind(resource.id.to_string())
            .fetch_one(db.pool())
            .await
            .expect("count lifecycle audit events");
    assert_eq!(events, 2);
}
