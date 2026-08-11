mod support;

use std::collections::HashMap;

use conductor_domain::{
    CreateResourceRequest, PrimaryRole, ReleaseChannel, ReleaseResourceRequest, ResourceKind,
    ResourceVisibility, SetupRequest, VersionMode,
};
use conductor_storage::repos::ReleaseContent;
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
