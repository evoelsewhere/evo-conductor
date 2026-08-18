use std::time::Duration;

use chrono::Utc;
use conductor_domain::PrimaryRole;
use conductor_storage::{ChangeMemberStatus, Db, MemberAccessError, UpdateAccessProfile};
use sqlx::Row;
use uuid::Uuid;

const LOCK_OBSERVATION_TIMEOUT: Duration = Duration::from_secs(5);
const MUTATION_COMPLETION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
enum MemberMutation {
    Demote,
    Disable,
}

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
            ) VALUES ($1, 'PostgreSQL member access test', '127.0.0.1', 0, 'L1', 1, 'unused', $2, $3)
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
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id.to_string())
            .execute(db.pool())
            .await
            .expect("remove PostgreSQL fixture user");
    }
}

async fn seed_active_admin(db: &Db) -> Uuid {
    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO users (
            id, email, display_name, primary_role, status, created_at
        ) VALUES ($1, $2, 'PostgreSQL Admin', 'admin', 'active', $3)
        "#,
    )
    .bind(id.to_string())
    .bind(format!("admin-{}@example.test", id.simple()))
    .bind(now)
    .execute(db.pool())
    .await
    .expect("seed active PostgreSQL admin");
    id
}

async fn wait_for_project_lock_waiter(db: &Db) -> i64 {
    tokio::time::timeout(LOCK_OBSERVATION_TIMEOUT, async {
        loop {
            let row = sqlx::query(
                r#"
                SELECT activity.pid::BIGINT AS pid
                FROM pg_stat_activity AS activity
                WHERE activity.datname = current_database()
                  AND activity.usename = current_user
                  AND activity.pid <> pg_backend_pid()
                  AND activity.state = 'active'
                  AND activity.wait_event_type = 'Lock'
                  AND activity.query LIKE '%SELECT id FROM instance%'
                  AND activity.query LIKE '%FOR UPDATE%'
                  AND EXISTS (
                      SELECT 1
                      FROM pg_locks AS locks
                      WHERE locks.pid = activity.pid
                        AND locks.granted = FALSE
                  )
                ORDER BY activity.query_start DESC
                LIMIT 1
                "#,
            )
            .fetch_optional(db.pool())
            .await
            .expect("inspect PostgreSQL lock waiters");

            if let Some(row) = row {
                return row.get("pid");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("second PostgreSQL backend did not wait on the singleton project row lock")
}

async fn assert_second_waits_then_rejects_last_admin(
    db: &Db,
    first_mutation: MemberMutation,
    second_mutation: MemberMutation,
) {
    let admin_a = seed_active_admin(db).await;
    let admin_b = seed_active_admin(db).await;

    let mut first = db.pool().begin().await.expect("begin first transaction");
    sqlx::query("SELECT id FROM instance ORDER BY created_at ASC LIMIT 1 FOR UPDATE")
        .fetch_one(&mut *first)
        .await
        .expect("lock singleton project row");
    match first_mutation {
        MemberMutation::Demote => {
            sqlx::query("UPDATE users SET primary_role = 'user' WHERE id = $1")
                .bind(admin_b.to_string())
                .execute(&mut *first)
                .await
                .expect("stage first demotion");
        }
        MemberMutation::Disable => {
            sqlx::query(
                "UPDATE users SET status = 'disabled', session_version = session_version + 1 \
                 WHERE id = $1",
            )
            .bind(admin_b.to_string())
            .execute(&mut *first)
            .await
            .expect("stage first disable");
        }
    }

    let repo = db.member_access();
    let mut second = tokio::spawn(async move {
        match second_mutation {
            MemberMutation::Demote => {
                repo.update_access_profile(UpdateAccessProfile {
                    actor_id: admin_b,
                    target_id: admin_a,
                    display_name: None,
                    primary_role: Some(PrimaryRole::User),
                    sub_role_ids: None,
                    tag_ids: None,
                })
                .await
            }
            MemberMutation::Disable => {
                repo.set_member_status(ChangeMemberStatus::disable(admin_b, admin_a))
                    .await
            }
        }
    });

    let waiting_backend_pid = wait_for_project_lock_waiter(db).await;
    assert!(
        waiting_backend_pid > 0,
        "PostgreSQL must report a physical backend waiting on the project row lock"
    );
    first.commit().await.expect("commit first transaction");

    let joined = match tokio::time::timeout(MUTATION_COMPLETION_TIMEOUT, &mut second).await {
        Ok(joined) => joined,
        Err(_) => {
            second.abort();
            panic!("second mutation did not complete after the project lock was released");
        }
    };
    let error = joined
        .expect("second task joined")
        .expect_err("second mutation must reject removal of the last active Admin");
    assert!(
        matches!(&error, MemberAccessError::LastActiveAdmin),
        "expected LastActiveAdmin after lock handoff, got {error:?}"
    );
    let active_admins: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM users WHERE primary_role = 'admin' AND status = 'active'",
    )
    .fetch_one(db.pool())
    .await
    .expect("count active admins")
    .get("count");
    assert_eq!(active_admins, 1);

    delete_fixture_users(db, [admin_a, admin_b]).await;
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
async fn postgres_project_lock_serializes_demote_and_mixed_interleavings() {
    let url = std::env::var("CONDUCTOR_TEST_POSTGRES_URL")
        .expect("CONDUCTOR_TEST_POSTGRES_URL is required for the PostgreSQL locking proof");
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "CONDUCTOR_TEST_POSTGRES_URL must be a PostgreSQL URL"
    );
    let db = Db::connect(&url).await.expect("connect PostgreSQL test DB");
    ensure_disposable_database(&db).await;

    assert_second_waits_then_rejects_last_admin(
        &db,
        MemberMutation::Demote,
        MemberMutation::Demote,
    )
    .await;
    assert_second_waits_then_rejects_last_admin(
        &db,
        MemberMutation::Demote,
        MemberMutation::Disable,
    )
    .await;
}
