//! The memo in `InstanceRepo::authorization_project_id` is only safe because
//! `instance.id` is immutable after setup. These tests pin the properties that
//! make it safe, so breaking one fails here rather than in authorization.

mod support;

use conductor_domain::SetupRequest;
use conductor_storage::Db;
use support::{connect_test_db, test_database_url};

const PLACEHOLDER_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c3RlcDBzYWx0$0000000000000000000000000000000000000000000";

async fn complete_setup(db: &Db) -> uuid::Uuid {
    let (instance, _admin) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "Memo Project".into(),
                display_name: Some("Memo".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "admin@example.test".into(),
                admin_display_name: "Admin".into(),
                admin_password: "correct horse battery staple".into(),
                sso: None,
            },
            PLACEHOLDER_HASH,
            "test-jwt-secret",
            None,
        )
        .await
        .expect("complete setup");
    instance.id
}

/// The property that matters most: memoizing "no project" would strand a
/// server that completes setup in this same process as unconfigured.
#[tokio::test]
async fn absent_project_is_not_memoized_so_setup_is_observed() {
    let db = connect_test_db().await;

    assert_eq!(
        db.instance()
            .authorization_project_id()
            .await
            .expect("query before setup"),
        None,
        "a database without an instance row must report no project"
    );

    let project_id = complete_setup(&db).await;

    assert_eq!(
        db.instance()
            .authorization_project_id()
            .await
            .expect("query after setup"),
        Some(project_id),
        "setup completed in-process must be picked up on the next call"
    );
}

/// Deleting the row is the only way to observe from outside that no second
/// query happened. It is not a supported operation: `make reset-db` already
/// tells the operator to restart the API first.
#[tokio::test]
async fn resolved_project_id_is_served_from_the_memo() {
    let db = connect_test_db().await;
    let project_id = complete_setup(&db).await;

    assert_eq!(
        db.instance().authorization_project_id().await.unwrap(),
        Some(project_id)
    );

    sqlx::query("DELETE FROM instance")
        .execute(db.pool())
        .await
        .expect("delete the instance row");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance")
        .fetch_one(db.pool())
        .await
        .expect("count instance rows");
    assert_eq!(remaining, 0, "the row this test relies on removing is gone");

    assert_eq!(
        db.instance().authorization_project_id().await.unwrap(),
        Some(project_id),
        "a resolved project id must come from the memo, not a fresh query"
    );
}

/// The memo lives on the handle, which is what keeps tests (and any ad hoc
/// `InstanceRepo::new`) isolated from each other.
#[tokio::test]
async fn a_separate_db_handle_does_not_inherit_the_memo() {
    let url = test_database_url();
    let first = Db::connect(&url).await.expect("first handle");
    let project_id = complete_setup(&first).await;
    assert_eq!(
        first.instance().authorization_project_id().await.unwrap(),
        Some(project_id)
    );

    let second = Db::connect(&url).await.expect("second handle");
    assert_eq!(
        second.instance().authorization_project_id().await.unwrap(),
        Some(project_id),
        "a second handle must resolve the same persisted id"
    );
}

/// The memo must not smuggle a value past the "exactly one project identity"
/// check.
#[tokio::test]
async fn two_project_rows_still_fail_closed_when_nothing_is_memoized() {
    let url = test_database_url();
    let seeder = Db::connect(&url).await.expect("seeding handle");
    complete_setup(&seeder).await;

    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, bind_host, bind_port, collection_level,
            storage_backend, storage_config, setup_completed, jwt_secret,
            created_at, updated_at
        ) VALUES (?, 'Second Project', '127.0.0.1', 4701, 'L1', 'local', '{}', 1,
                  'second-secret', '2999-01-01T00:00:00Z', '2999-01-01T00:00:00Z')
        "#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(seeder.pool())
    .await
    .expect("insert a second project row");

    let fresh = Db::connect(&url).await.expect("handle with an empty memo");
    let result = fresh.instance().authorization_project_id().await;
    assert!(
        result.is_err(),
        "two project rows must be rejected, got {:?}",
        result.ok()
    );
}
