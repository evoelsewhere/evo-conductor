//! Migrations must apply cleanly to an empty database and be safe to re-run.
//!
//! These tests are the ones REQ-001 will extend to PostgreSQL. They currently
//! exercise the runtime `CREATE TABLE IF NOT EXISTS` mechanism, which discards
//! `ALTER TABLE` errors and tracks no schema version.

use conductor_storage::Db;
use sqlx::Row;
use uuid::Uuid;

fn fresh_url() -> String {
    format!(
        "sqlite:file:conductor_test_{}?mode=memory&cache=shared",
        Uuid::new_v4().simple()
    )
}

/// Every table the current schema is expected to create.
const EXPECTED_TABLES: &[&str] = &[
    "instance",
    "sso_config",
    "users",
    "sub_roles",
    "user_sub_roles",
    "tags",
    "user_tags",
    "tag_assignments",
    "connection_secrets",
    "resources",
    "member_inventory",
    "telemetry_events",
];

async fn table_names(db: &Db) -> Vec<String> {
    sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .fetch_all(db.pool())
        .await
        .expect("list tables")
        .into_iter()
        .map(|r| r.get::<String, _>("name"))
        .collect()
}

#[tokio::test]
async fn migrations_apply_to_an_empty_database() {
    let db = Db::connect(&fresh_url()).await.expect("connect");
    let tables = table_names(&db).await;

    for expected in EXPECTED_TABLES {
        assert!(
            tables.iter().any(|t| t == expected),
            "table `{expected}` missing after migration; found {tables:?}"
        );
    }
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let url = fresh_url();
    let db = Db::connect(&url).await.expect("first connect");
    let before = table_names(&db).await;

    // Connecting again re-runs migrate::run against the same database.
    let db2 = Db::connect(&url).await.expect("second connect");
    let after = table_names(&db2).await;

    assert_eq!(before, after, "re-running migrations changed the schema");
}

/// The indexes created alongside the tables are what fail first when the pool
/// hands out separate databases, so assert they exist.
#[tokio::test]
async fn migrations_create_the_declared_indexes() {
    let db = Db::connect(&fresh_url()).await.expect("connect");

    let indexes: Vec<String> = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_%' ORDER BY name",
    )
    .fetch_all(db.pool())
    .await
    .expect("list indexes")
    .into_iter()
    .map(|r| r.get::<String, _>("name"))
    .collect();

    for expected in [
        "idx_users_status",
        "idx_users_primary_role",
        "idx_user_tags_tag",
        "idx_tag_assignments_entity",
        "idx_tag_assignments_tag",
        "idx_user_sub_roles_role",
    ] {
        assert!(
            indexes.iter().any(|i| i == expected),
            "index `{expected}` missing; found {indexes:?}"
        );
    }
}

/// `telemetry_events` has no index at all, which REQ-014 must fix. This test
/// records the current state so the gap is visible rather than assumed.
#[tokio::test]
async fn telemetry_events_currently_has_no_index() {
    let db = Db::connect(&fresh_url()).await.expect("connect");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND tbl_name = 'telemetry_events' AND name LIKE 'idx_%'",
    )
    .fetch_one(db.pool())
    .await
    .expect("count telemetry indexes");

    assert_eq!(
        count, 0,
        "telemetry_events gained an index — update REQ-014 and remove this test"
    );
}
