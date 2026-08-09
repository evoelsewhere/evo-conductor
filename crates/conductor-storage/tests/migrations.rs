//! Migrations must apply cleanly to an empty database and be safe to re-run.
//!
//! The expected tables and indexes are declared in
//! `conductor_storage::core::constants`, next to the migration that creates
//! them, so this file asserts against the schema rather than a copy of it.

mod support;

use conductor_storage::core::constants::schema::{INDEXES, TABLES};
use conductor_storage::Db;
use sqlx::Row;
use support::test_database_url;

/// The table REQ-014 must index. It currently has none.
const UNINDEXED_TABLE: &str = "telemetry_events";

const LIST_TABLES: &str = "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name";
const LIST_INDEXES: &str =
    "SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_%' ORDER BY name";
const COUNT_INDEXES_ON_TABLE: &str = "SELECT COUNT(*) FROM sqlite_master \
     WHERE type = 'index' AND tbl_name = ? AND name LIKE 'idx_%'";

async fn names(db: &Db, query: &str) -> Vec<String> {
    sqlx::query(query)
        .fetch_all(db.pool())
        .await
        .expect("query sqlite_master")
        .into_iter()
        .map(|r| r.get::<String, _>("name"))
        .collect()
}

#[tokio::test]
async fn migrations_apply_to_an_empty_database() {
    let db = Db::connect(&test_database_url()).await.expect("connect");
    let tables = names(&db, LIST_TABLES).await;

    for expected in TABLES {
        assert!(
            tables.iter().any(|t| t == expected),
            "table `{expected}` missing after migration; found {tables:?}"
        );
    }
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let url = test_database_url();
    let first = Db::connect(&url).await.expect("first connect");
    let before = names(&first, LIST_TABLES).await;

    // Connecting again re-runs migrate::run against the same database.
    let second = Db::connect(&url).await.expect("second connect");
    let after = names(&second, LIST_TABLES).await;

    assert_eq!(before, after, "re-running migrations changed the schema");
}

#[tokio::test]
async fn migrations_create_the_declared_indexes() {
    let db = Db::connect(&test_database_url()).await.expect("connect");
    let indexes = names(&db, LIST_INDEXES).await;

    for expected in INDEXES {
        assert!(
            indexes.iter().any(|i| i == expected),
            "index `{expected}` missing; found {indexes:?}"
        );
    }
}

/// Records that `telemetry_events` has no index, so the gap REQ-014 must close
/// is visible rather than assumed. Fails once the gap closes, which is the
/// signal to delete this reminder.
#[tokio::test]
async fn telemetry_events_currently_has_no_index() {
    let db = Db::connect(&test_database_url()).await.expect("connect");

    let count: i64 = sqlx::query_scalar(COUNT_INDEXES_ON_TABLE)
        .bind(UNINDEXED_TABLE)
        .fetch_one(db.pool())
        .await
        .expect("count telemetry indexes");

    assert_eq!(
        count, 0,
        "{UNINDEXED_TABLE} gained an index — REQ-014 is done, remove this reminder"
    );
}
