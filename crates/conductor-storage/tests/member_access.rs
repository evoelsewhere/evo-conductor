mod support;

use std::time::Duration;

use chrono::Utc;
use conductor_domain::{
    CreateMemberRequest, CreateSubRoleRequest, CreateTagRequest, PrimaryRole, SecretScope,
    UserStatus,
};
use conductor_storage::{
    ApproveMemberAccess, ChangeMemberStatus, Db, MemberAccessError, UpdateAccessProfile,
};
use sqlx::Row;
use uuid::Uuid;

async fn seed_instance(db: &Db) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, bind_host, bind_port, collection_level,
            setup_completed, jwt_secret, created_at, updated_at
        ) VALUES (?, 'Member access test', '127.0.0.1', 0, 'L1', 1, 'unused', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&now)
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("seed singleton instance");
}

async fn create_sub_role(db: &Db, slug: &str) -> String {
    db.roles()
        .create_sub_role(&CreateSubRoleRequest {
            slug: slug.into(),
            name: slug.into(),
            description: None,
            color: None,
        })
        .await
        .expect("create sub-role")
        .id
}

async fn create_tag(db: &Db, slug: &str) -> String {
    db.roles()
        .create_tag(&CreateTagRequest {
            slug: slug.into(),
            name: slug.into(),
            description: None,
            color: None,
        })
        .await
        .expect("create tag")
        .id
}

fn profile(
    actor_id: Uuid,
    target_id: Uuid,
    primary_role: Option<PrimaryRole>,
    sub_role_ids: Option<Vec<String>>,
    tag_ids: Option<Vec<String>>,
) -> UpdateAccessProfile {
    UpdateAccessProfile {
        actor_id,
        target_id,
        display_name: None,
        primary_role,
        sub_role_ids,
        tag_ids,
    }
}

#[tokio::test]
async fn role_assignments_and_session_version_commit_as_one_profile() {
    let db = support::connect_test_db().await;
    seed_instance(&db).await;
    let actor = support::seed_active_user(&db, PrimaryRole::Admin).await;
    let target = support::seed_active_user(&db, PrimaryRole::User).await;
    let sub_role_id = create_sub_role(&db, "platform").await;
    let tag_id = create_tag(&db, "backend").await;
    let secret = db
        .secrets()
        .insert(
            target.id,
            "desktop",
            "ef_test",
            "member-access-compatible-token",
            &[
                SecretScope::SubscribeResources,
                SecretScope::ReportTelemetry,
                SecretScope::SyncInventory,
            ],
            None,
        )
        .await
        .expect("insert compatible credential");
    let initial_version = db
        .users()
        .session_version(target.id)
        .await
        .expect("read session version")
        .expect("target exists");

    let promoted = db
        .member_access()
        .update_access_profile(profile(
            actor.id,
            target.id,
            Some(PrimaryRole::Contribute),
            Some(vec![sub_role_id.clone()]),
            Some(vec![tag_id.clone()]),
        ))
        .await
        .expect("promote to contributor");
    assert_eq!(promoted.user.primary_role, PrimaryRole::Contribute);
    assert_eq!(promoted.user.sub_role_ids, vec![sub_role_id]);
    assert_eq!(promoted.user.tag_ids, vec![tag_id]);
    assert_eq!(promoted.change.before.session_version, initial_version);
    assert_eq!(promoted.change.after.session_version, initial_version);
    assert!(!promoted.change.admin_elevation);
    assert!(promoted.change.audience_changed);
    assert!(promoted.change.revoked_credentials.is_empty());

    let demoted = db
        .member_access()
        .update_access_profile(profile(
            actor.id,
            target.id,
            Some(PrimaryRole::User),
            None,
            None,
        ))
        .await
        .expect("demote to user");
    assert_eq!(demoted.change.after.session_version, initial_version);

    let elevated = db
        .member_access()
        .update_access_profile(profile(
            actor.id,
            target.id,
            Some(PrimaryRole::Admin),
            None,
            None,
        ))
        .await
        .expect("elevate to admin");
    assert!(elevated.change.admin_elevation);
    assert_eq!(elevated.change.after.session_version, initial_version + 1);

    let demoted_again = db
        .member_access()
        .update_access_profile(profile(
            actor.id,
            target.id,
            Some(PrimaryRole::User),
            None,
            None,
        ))
        .await
        .expect("demote from admin");
    assert_eq!(
        demoted_again.change.after.session_version,
        initial_version + 1,
        "demotion must use the current browser credential"
    );
    assert!(db
        .secrets()
        .is_active(secret.id)
        .await
        .expect("read credential state"));
}

#[tokio::test]
async fn injected_junction_failure_rolls_back_user_and_every_assignment_write() {
    let db = support::connect_test_db().await;
    seed_instance(&db).await;
    let actor = support::seed_active_user(&db, PrimaryRole::Admin).await;
    let target = support::seed_active_user(&db, PrimaryRole::User).await;
    let old_sub_role = create_sub_role(&db, "old-sub").await;
    let next_sub_role = create_sub_role(&db, "next-sub").await;
    let old_tag = create_tag(&db, "old-tag").await;
    let first_tag = create_tag(&db, "first-tag").await;
    let failing_tag = create_tag(&db, "failing-tag").await;

    db.member_access()
        .update_access_profile(profile(
            actor.id,
            target.id,
            None,
            Some(vec![old_sub_role.clone()]),
            Some(vec![old_tag.clone()]),
        ))
        .await
        .expect("seed old access profile");
    let before_version = db
        .users()
        .session_version(target.id)
        .await
        .expect("read version")
        .expect("target exists");

    let trigger_sql = format!(
        "CREATE TRIGGER fail_member_tag_insert BEFORE INSERT ON tag_assignments \
         WHEN NEW.entity_type = 'member' AND NEW.entity_id = '{}' AND NEW.tag_id = '{}' \
         BEGIN SELECT RAISE(ABORT, 'injected member tag failure'); END",
        target.id, failing_tag
    );
    sqlx::query(&trigger_sql)
        .execute(db.pool())
        .await
        .expect("install failure trigger");

    let error = db
        .member_access()
        .update_access_profile(UpdateAccessProfile {
            actor_id: actor.id,
            target_id: target.id,
            display_name: Some("Changed but rolled back".into()),
            primary_role: Some(PrimaryRole::Contribute),
            sub_role_ids: Some(vec![next_sub_role]),
            tag_ids: Some(vec![first_tag, failing_tag]),
        })
        .await
        .expect_err("trigger must abort the profile transaction");
    assert!(matches!(error, MemberAccessError::Database(_)));

    let after = db
        .users()
        .find_by_id(target.id)
        .await
        .expect("read target")
        .expect("target exists");
    assert_eq!(after.display_name, target.display_name);
    assert_eq!(after.primary_role, PrimaryRole::User);
    assert_eq!(after.sub_role_ids, vec![old_sub_role]);
    assert_eq!(after.tag_ids, vec![old_tag]);
    assert_eq!(
        db.users()
            .session_version(target.id)
            .await
            .expect("read version")
            .expect("target exists"),
        before_version
    );
}

#[tokio::test]
async fn invalid_stored_credential_rolls_back_the_complete_profile() {
    let db = support::connect_test_db().await;
    seed_instance(&db).await;
    let actor = support::seed_active_user(&db, PrimaryRole::Admin).await;
    let target = support::seed_active_user(&db, PrimaryRole::User).await;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO connection_secrets (
            id, name, prefix, token_hash, owner_user_id, scopes, created_at
        ) VALUES (?, 'corrupt', 'bad', 'bad-hash', ?, '[]', ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(target.id.to_string())
    .bind(now)
    .execute(db.pool())
    .await
    .expect("insert malformed credential fixture");

    let error = db
        .member_access()
        .update_access_profile(profile(
            actor.id,
            target.id,
            Some(PrimaryRole::Contribute),
            Some(vec![]),
            Some(vec![]),
        ))
        .await
        .expect_err("invalid stored credential must fail closed");
    assert!(matches!(
        error,
        MemberAccessError::InvalidPersistedCredential(_)
    ));
    let after = db
        .users()
        .find_by_id(target.id)
        .await
        .expect("read target")
        .expect("target exists");
    assert_eq!(after.primary_role, PrimaryRole::User);
}

#[tokio::test]
async fn approval_status_role_and_assignments_commit_or_roll_back_together() {
    let db = support::connect_test_db().await;
    seed_instance(&db).await;
    let actor = support::seed_active_user(&db, PrimaryRole::Admin).await;
    let target = db
        .users()
        .create_invited(
            &CreateMemberRequest {
                email: format!("approval-{}@example.test", Uuid::new_v4().simple()),
                display_name: "Pending approval".into(),
                primary_role: PrimaryRole::User,
                sub_role_ids: vec![],
                tag_ids: vec![],
            },
            support::PLACEHOLDER_PASSWORD_HASH,
            actor.id,
        )
        .await
        .expect("create invited target");
    let sub_role_id = create_sub_role(&db, "approval-sub").await;
    let tag_id = create_tag(&db, "approval-tag").await;
    let trigger_sql = format!(
        "CREATE TRIGGER fail_approval_tag BEFORE INSERT ON tag_assignments \
         WHEN NEW.entity_type = 'member' AND NEW.entity_id = '{}' AND NEW.tag_id = '{}' \
         BEGIN SELECT RAISE(ABORT, 'injected approval failure'); END",
        target.id, tag_id
    );
    sqlx::query(&trigger_sql)
        .execute(db.pool())
        .await
        .expect("install approval failure trigger");

    let command = ApproveMemberAccess {
        actor_id: actor.id,
        target_id: target.id,
        primary_role: Some(PrimaryRole::Admin),
        sub_role_ids: Some(vec![sub_role_id.clone()]),
        tag_ids: Some(vec![tag_id.clone()]),
    };
    let error = db
        .member_access()
        .approve_member(command.clone())
        .await
        .expect_err("injected assignment failure must roll back approval");
    assert!(matches!(error, MemberAccessError::Database(_)));
    let rolled_back = db
        .users()
        .find_by_id(target.id)
        .await
        .expect("read target")
        .expect("target exists");
    assert_eq!(rolled_back.status, UserStatus::Invited);
    assert_eq!(rolled_back.primary_role, PrimaryRole::User);
    assert!(rolled_back.sub_role_ids.is_empty());
    assert!(rolled_back.tag_ids.is_empty());
    assert_eq!(
        db.users()
            .session_version(target.id)
            .await
            .expect("read version")
            .expect("target exists"),
        0
    );

    sqlx::query("DROP TRIGGER fail_approval_tag")
        .execute(db.pool())
        .await
        .expect("remove approval failure trigger");
    let approved = db
        .member_access()
        .approve_member(command)
        .await
        .expect("approve target atomically");
    assert_eq!(approved.user.status, UserStatus::Active);
    assert_eq!(approved.user.primary_role, PrimaryRole::Admin);
    assert_eq!(approved.user.sub_role_ids, vec![sub_role_id]);
    assert_eq!(approved.user.tag_ids, vec![tag_id]);
    assert_eq!(approved.change.after.session_version, 1);
    assert!(approved.change.admin_elevation);
}

#[tokio::test]
async fn disable_and_enable_share_the_lock_bump_sessions_and_do_not_revoke_v1_tokens() {
    let db = support::connect_test_db().await;
    seed_instance(&db).await;
    let actor = support::seed_active_user(&db, PrimaryRole::Admin).await;
    let target = support::seed_active_user(&db, PrimaryRole::User).await;
    let secret = db
        .secrets()
        .insert(
            target.id,
            "desktop",
            "ef_status",
            "member-status-token",
            &[SecretScope::SubscribeResources],
            None,
        )
        .await
        .expect("insert credential");
    let initial_version = db
        .users()
        .session_version(target.id)
        .await
        .expect("read version")
        .expect("target exists");

    let disabled = db
        .member_access()
        .set_member_status(ChangeMemberStatus::disable(actor.id, target.id))
        .await
        .expect("disable target");
    assert_eq!(disabled.user.status, UserStatus::Disabled);
    assert_eq!(disabled.change.after.session_version, initial_version + 1);
    assert!(disabled.change.revoked_credentials.is_empty());
    assert!(db
        .secrets()
        .is_active(secret.id)
        .await
        .expect("REQ-005 has not revoked the credential"));

    let enabled = db
        .member_access()
        .set_member_status(ChangeMemberStatus::enable(actor.id, target.id))
        .await
        .expect("enable target");
    assert_eq!(enabled.user.status, UserStatus::Active);
    assert_eq!(enabled.change.after.session_version, initial_version + 2);
    assert!(db
        .secrets()
        .is_active(secret.id)
        .await
        .expect("REQ-005 boundary remains explicit"));
}

#[tokio::test]
async fn concurrent_profile_and_status_changes_cannot_remove_every_active_admin() {
    let path = std::env::temp_dir().join(format!(
        "evo-conductor-member-access-{}.db",
        Uuid::new_v4().simple()
    ));
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let db = Db::connect(&url).await.expect("connect file-backed SQLite");
    seed_instance(&db).await;
    let admin_a = support::seed_active_user(&db, PrimaryRole::Admin).await;
    let admin_b = support::seed_active_user(&db, PrimaryRole::Admin).await;

    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
    let first_barrier = barrier.clone();
    let first_repo = db.member_access();
    let first = tokio::spawn(async move {
        first_barrier.wait().await;
        first_repo
            .update_access_profile(profile(
                admin_a.id,
                admin_b.id,
                Some(PrimaryRole::User),
                None,
                None,
            ))
            .await
    });
    let second_barrier = barrier.clone();
    let second_repo = db.member_access();
    let second = tokio::spawn(async move {
        second_barrier.wait().await;
        second_repo
            .set_member_status(ChangeMemberStatus::disable(admin_b.id, admin_a.id))
            .await
    });
    barrier.wait().await;

    let (first, second) = tokio::time::timeout(Duration::from_secs(5), async {
        (
            first.await.expect("profile task"),
            second.await.expect("status task"),
        )
    })
    .await
    .expect("serialized operations complete");
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let rejected = first.err().or_else(|| second.err()).expect("one rejection");
    assert!(matches!(
        rejected,
        MemberAccessError::LastActiveAdmin | MemberAccessError::ActorNotAuthorized
    ));
    let active_admins: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM users WHERE primary_role = 'admin' AND status = 'active'",
    )
    .fetch_one(db.pool())
    .await
    .expect("count active admins")
    .get("count");
    assert_eq!(active_admins, 1);

    db.pool().close().await;
    for candidate in [
        path.clone(),
        path.with_extension("db-wal"),
        path.with_extension("db-shm"),
    ] {
        let _ = std::fs::remove_file(candidate);
    }
}
