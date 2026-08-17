mod support;

use std::time::Duration;

use chrono::Utc;
use conductor_domain::PrimaryRole;
use conductor_storage::{ChangeMemberStatus, Db, MemberAccessError, UpdateAccessProfile};
use sqlx::Row;
use uuid::Uuid;

async fn ensure_disposable_database(db: &Db) {
    let user_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM users")
        .fetch_one(db.pool())
        .await
        .expect("count existing users")
        .get("count");
    assert_eq!(
        user_count, 0,
        "CONDUCTOR_TEST_POSTGRES_URL must identify an empty disposable database"
    );

    let instance_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM instance")
        .fetch_one(db.pool())
        .await
        .expect("count instances")
        .get("count");
    if instance_count == 0 {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO instance (
                id, project_name, bind_host, bind_port, collection_level,
                setup_completed, jwt_secret, created_at, updated_at
            ) VALUES (?, 'PostgreSQL member access test', '127.0.0.1', 0, 'L1', 1, 'unused', ?, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&now)
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("seed singleton instance");
    } else {
        assert_eq!(
            instance_count, 1,
            "member security lock requires exactly one project instance"
        );
    }
}

async fn delete_fixture_users(db: &Db, user_ids: [Uuid; 2]) {
    for user_id in user_ids {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id.to_string())
            .execute(db.pool())
            .await
            .expect("remove PostgreSQL fixture user");
    }
}

async fn assert_second_waits_then_rejects_last_admin(db: &Db, first_disables: bool) {
    let admin_a = support::seed_active_user(db, PrimaryRole::Admin).await;
    let admin_b = support::seed_active_user(db, PrimaryRole::Admin).await;

    let mut first = db.pool().begin().await.expect("begin first transaction");
    sqlx::query("SELECT id FROM instance ORDER BY created_at ASC LIMIT 1 FOR UPDATE")
        .fetch_one(&mut *first)
        .await
        .expect("lock singleton project row");
    if first_disables {
        sqlx::query(
            "UPDATE users SET status = 'disabled', session_version = session_version + 1 \
             WHERE id = ?",
        )
        .bind(admin_b.id.to_string())
        .execute(&mut *first)
        .await
        .expect("stage first disable");
    } else {
        sqlx::query("UPDATE users SET primary_role = 'user' WHERE id = ?")
            .bind(admin_b.id.to_string())
            .execute(&mut *first)
            .await
            .expect("stage first demotion");
    }

    let repo = db.member_access();
    let second = tokio::spawn(async move {
        if first_disables {
            repo.update_access_profile(UpdateAccessProfile {
                actor_id: admin_b.id,
                target_id: admin_a.id,
                display_name: None,
                primary_role: Some(PrimaryRole::User),
                sub_role_ids: None,
                tag_ids: None,
            })
            .await
        } else {
            repo.set_member_status(ChangeMemberStatus::disable(admin_b.id, admin_a.id))
                .await
        }
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !second.is_finished(),
        "the second physical connection must wait for the singleton row lock"
    );
    first.commit().await.expect("commit first transaction");

    let error = second
        .await
        .expect("second task joined")
        .expect_err("second mutation must reject removal of the last active Admin");
    assert!(matches!(error, MemberAccessError::LastActiveAdmin));
    let active_admins: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM users WHERE primary_role = 'admin' AND status = 'active'",
    )
    .fetch_one(db.pool())
    .await
    .expect("count active admins")
    .get("count");
    assert_eq!(active_admins, 1);

    delete_fixture_users(db, [admin_a.id, admin_b.id]).await;
}

/// Run explicitly with:
/// `CONDUCTOR_TEST_POSTGRES_URL=postgres://... cargo test -p conductor-storage \
///   --test member_access_postgres -- --ignored`
///
/// The test is ignored in the backend's default local suite because it requires
/// a real, empty, disposable PostgreSQL database. When explicitly selected, a
/// missing URL or non-empty database fails loudly instead of silently skipping.
#[tokio::test]
#[ignore = "requires CONDUCTOR_TEST_POSTGRES_URL pointing to an empty disposable PostgreSQL database"]
async fn postgres_project_lock_serializes_demote_disable_interleavings() {
    let url = std::env::var("CONDUCTOR_TEST_POSTGRES_URL")
        .expect("CONDUCTOR_TEST_POSTGRES_URL is required for the PostgreSQL locking proof");
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "CONDUCTOR_TEST_POSTGRES_URL must be a PostgreSQL URL"
    );
    let db = Db::connect(&url).await.expect("connect PostgreSQL test DB");
    ensure_disposable_database(&db).await;

    assert_second_waits_then_rejects_last_admin(&db, false).await;
    assert_second_waits_then_rejects_last_admin(&db, true).await;
}
