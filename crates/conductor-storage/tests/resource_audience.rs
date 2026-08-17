mod support;

use conductor_domain::{
    CreateResourceRequest, PrimaryRole, ResourceAccessPolicy, ResourceKind, ResourceVisibility,
    SetupRequest,
};
use conductor_storage::repos::DraftContent;
use conductor_storage::{StorageError, UpdateAccessProfile};
use support::{connect_test_db, seed_active_user, PLACEHOLDER_PASSWORD_HASH};

#[tokio::test]
async fn member_tag_assignments_drive_resource_visibility() {
    let db = connect_test_db().await;
    let (project, owner) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "tag-audience-test".into(),
                display_name: Some("Tag audience test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "tag-audience-admin@example.test".into(),
                admin_display_name: "Tag Audience Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            PLACEHOLDER_PASSWORD_HASH,
            "tag-audience-jwt-secret",
            None,
        )
        .await
        .expect("complete setup");
    let member = seed_active_user(&db, PrimaryRole::User).await;
    let tag_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO tags (id, slug, name, created_at) VALUES (?, 'platform', 'Platform', ?)",
    )
    .bind(&tag_id)
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("seed tag");

    let resource = db
        .resources()
        .create(
            project.id,
            &CreateResourceRequest {
                kind: ResourceKind::Agent,
                slug: "tagged-agent".into(),
                name: "Tagged agent".into(),
                description: None,
                version: "0.1.0".into(),
                visibility: ResourceVisibility::Private,
                payload: serde_json::json!({}),
                changelog: None,
            },
            owner.id,
            &DraftContent {
                artifact_key:
                    "sha256/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .into(),
                sha256: "a".repeat(64),
                size: 1,
                metadata_payload: serde_json::json!({}),
            },
        )
        .await
        .expect("create resource");
    sqlx::query("UPDATE resources SET status = 'published' WHERE id = ?")
        .bind(resource.id.to_string())
        .execute(db.pool())
        .await
        .expect("publish fixture resource");
    db.resources()
        .set_access_policy(
            resource.id,
            &ResourceAccessPolicy {
                tag_ids: vec![tag_id.clone()],
                ..ResourceAccessPolicy::default()
            },
        )
        .await
        .expect("set tag audience");

    assert!(!db
        .resources()
        .visible_resource_ids(member.id)
        .await
        .expect("visibility before assignment")
        .contains(&resource.id));

    db.member_access()
        .update_access_profile(UpdateAccessProfile {
            actor_id: owner.id,
            target_id: member.id,
            display_name: None,
            primary_role: None,
            sub_role_ids: None,
            tag_ids: Some(vec![tag_id.clone()]),
        })
        .await
        .expect("assign member tag through canonical member path");

    assert!(db
        .resources()
        .visible_resource_ids(member.id)
        .await
        .expect("visibility after assignment")
        .contains(&resource.id));
}

#[tokio::test]
async fn invalid_effect_and_foreign_project_rows_never_grant_resource_visibility() {
    let db = connect_test_db().await;
    let (project, owner) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "audience-integrity-test".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "audience-integrity-admin@example.test".into(),
                admin_display_name: "Audience Integrity Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            PLACEHOLDER_PASSWORD_HASH,
            "audience-integrity-jwt-secret",
            None,
        )
        .await
        .expect("complete setup");
    let member = seed_active_user(&db, PrimaryRole::User).await;
    let resource = db
        .resources()
        .create(
            project.id,
            &CreateResourceRequest {
                kind: ResourceKind::Agent,
                slug: "audience-integrity-agent".into(),
                name: "Audience integrity agent".into(),
                description: None,
                version: "0.1.0".into(),
                visibility: ResourceVisibility::Private,
                payload: serde_json::json!({}),
                changelog: None,
            },
            owner.id,
            &DraftContent {
                artifact_key:
                    "sha256/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .into(),
                sha256: "a".repeat(64),
                size: 1,
                metadata_payload: serde_json::json!({}),
            },
        )
        .await
        .expect("create resource");
    sqlx::query("UPDATE resources SET status = 'published' WHERE id = ?")
        .bind(resource.id.to_string())
        .execute(db.pool())
        .await
        .expect("publish fixture resource");

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO resource_access_rules \
         (project_id, resource_id, subject_type, subject_id, effect, created_at) \
         VALUES (?, ?, 'all', '*', 'deny', ?)",
    )
    .bind(project.id.to_string())
    .bind(resource.id.to_string())
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("seed unsupported deny rule");
    assert!(!db
        .resources()
        .visible_resource_ids(member.id)
        .await
        .expect("unsupported effect fails closed")
        .contains(&resource.id));

    sqlx::query("UPDATE resource_access_rules SET effect = 'allow' WHERE resource_id = ?")
        .bind(resource.id.to_string())
        .execute(db.pool())
        .await
        .expect("make fixture rule valid");
    assert!(db
        .resources()
        .visible_resource_ids(member.id)
        .await
        .expect("valid allow rule grants visibility")
        .contains(&resource.id));

    let foreign_project = uuid::Uuid::new_v4();
    let mut connection = db
        .pool()
        .acquire()
        .await
        .expect("acquire database connection");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .expect("disable foreign keys for corrupt policy fixture");
    sqlx::query("UPDATE resource_access_rules SET project_id = ? WHERE resource_id = ?")
        .bind(foreign_project.to_string())
        .bind(resource.id.to_string())
        .execute(&mut *connection)
        .await
        .expect("move access rule to another project");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("restore foreign key enforcement");
    drop(connection);
    assert!(!db
        .resources()
        .visible_resource_ids(member.id)
        .await
        .expect("foreign-project allow rule is ignored")
        .contains(&resource.id));

    let mut connection = db
        .pool()
        .acquire()
        .await
        .expect("acquire database connection");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .expect("disable foreign keys for corrupt resource fixture");
    sqlx::query("UPDATE resources SET project_id = ? WHERE id = ?")
        .bind(foreign_project.to_string())
        .bind(resource.id.to_string())
        .execute(&mut *connection)
        .await
        .expect("move resource to another project");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("restore foreign key enforcement");
    drop(connection);
    assert!(!db
        .resources()
        .list_all()
        .await
        .expect("foreign-project resource is hidden")
        .iter()
        .any(|candidate| candidate.id == resource.id));
}

#[tokio::test]
async fn corrupt_change_rows_fail_closed_instead_of_advancing_the_cursor() {
    let db = connect_test_db().await;
    let (project, owner) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "change-integrity-test".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "change-integrity-admin@example.test".into(),
                admin_display_name: "Change Integrity Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            PLACEHOLDER_PASSWORD_HASH,
            "change-integrity-jwt-secret",
            None,
        )
        .await
        .expect("complete setup");

    let missing_resource = uuid::Uuid::new_v4();
    let mut connection = db.pool().acquire().await.expect("acquire connection");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .expect("disable foreign keys for corrupt change fixture");
    sqlx::query(
        "INSERT INTO resource_changes (sequence, project_id, resource_id, effective_user_id, change_kind, created_at) VALUES (1, ?, ?, NULL, 'archive', ?)",
    )
    .bind(project.id.to_string())
    .bind(missing_resource.to_string())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&mut *connection)
    .await
    .expect("seed corrupt change row");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("restore foreign keys");
    drop(connection);

    assert!(matches!(
        db.resources()
            .change_sequences(project.id, owner.id, 0, 10)
            .await,
        Err(StorageError::InvalidPersistedResource(_))
    ));
    assert!(matches!(
        db.resources()
            .max_change_sequence(project.id, owner.id)
            .await,
        Err(StorageError::InvalidPersistedResource(_))
    ));
}
