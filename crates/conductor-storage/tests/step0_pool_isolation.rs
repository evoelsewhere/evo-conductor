//! Step 0 of TSK-020-01: prove that an in-memory SQLite URL survives the
//! ten-connection pool that `Db::connect` hardcodes.
//!
//! Plain `sqlite::memory:` gives every pooled connection its own private
//! database, so `migrate::run` fails partway through. These tests decided the
//! database strategy in DES-020 section 2 before any fixture was written on top
//! of it, and they stay to keep that decision honest.

mod support;

use conductor_domain::PrimaryRole;
use conductor_storage::core::constants::database::{
    POOL_MAX_CONNECTIONS, SQLITE_MEMORY_PATH, SQLITE_SCHEME,
};
use conductor_storage::Db;
use sqlx::Row;
use support::{connect_test_db, seed_active_user};

/// The URL shape this file exists to characterise as unusable.
const PLAIN_MEMORY_URL: &str = concat!("sqlite:", ":memory:");

/// Enough simultaneous connections to exceed one while staying under the pool
/// maximum declared in `conductor_storage::core::constants`.
const CONCURRENT_CONNECTIONS: usize = 5;

/// Enough in-flight futures that the pool must spread them across connections.
const CONCURRENT_QUERIES: usize = 25;

const COUNT_USERS: &str = "SELECT COUNT(*) AS n FROM users";

/// Guards the two constants above against a change to the pool size.
#[test]
fn concurrency_figures_stay_within_the_pool() {
    assert!(CONCURRENT_CONNECTIONS < POOL_MAX_CONNECTIONS as usize);
    assert_eq!(
        PLAIN_MEMORY_URL,
        format!("{SQLITE_SCHEME}{SQLITE_MEMORY_PATH}")
    );
}

/// The control: record what plain `sqlite::memory:` actually does with this
/// pool, so the reason for the shared-cache URL is evidence rather than
/// folklore. If this ever starts behaving, the design note should be revisited.
#[tokio::test]
async fn plain_memory_url_is_unusable() {
    match Db::connect(PLAIN_MEMORY_URL).await {
        Err(e) => {
            println!("{PLAIN_MEMORY_URL} rejected at connect time: {e}");
        }
        Ok(db) => {
            // Hold two connections at once so the pool must open a second one.
            let mut a = db.pool().acquire().await.expect("acquire a");
            let mut b = db.pool().acquire().await.expect("acquire b");
            let on_a = sqlx::query(COUNT_USERS).fetch_one(&mut *a).await;
            let on_b = sqlx::query(COUNT_USERS).fetch_one(&mut *b).await;
            println!(
                "{PLAIN_MEMORY_URL} connected; first connection ok={}, second ok={}",
                on_a.is_ok(),
                on_b.is_ok()
            );
            assert!(
                on_a.is_err() || on_b.is_err(),
                "{PLAIN_MEMORY_URL} now works across pooled connections — \
                 revisit DES-020 section 2, the shared-cache workaround may be unnecessary"
            );
        }
    }
}

/// The decision: a named shared-cache database must be visible from every
/// connection the pool hands out.
#[tokio::test]
async fn shared_cache_url_is_visible_from_every_pooled_connection() {
    let db = connect_test_db().await;
    let user = seed_active_user(&db, PrimaryRole::User).await;

    let mut held = Vec::new();
    for _ in 0..CONCURRENT_CONNECTIONS {
        held.push(db.pool().acquire().await.expect("acquire"));
    }
    for (i, conn) in held.iter_mut().enumerate() {
        let row = sqlx::query(COUNT_USERS)
            .fetch_one(&mut **conn)
            .await
            .unwrap_or_else(|e| panic!("connection {i} could not see the users table: {e}"));
        let n: i64 = row.get("n");
        assert_eq!(n, 1, "connection {i} saw {n} users, expected 1");
    }
    drop(held);

    // And through two different repositories, which is how real tests use it.
    let found = db.users().find_by_id(user.id).await.expect("find_by_id");
    assert!(
        found.is_some(),
        "user written by one repo is unreadable by another"
    );
    assert_eq!(
        db.dashboard()
            .summary()
            .await
            .expect("summary")
            .members_total,
        1
    );
}

/// Concurrency is the case that breaks first in practice.
#[tokio::test]
async fn shared_cache_url_survives_concurrent_queries() {
    let db = connect_test_db().await;
    let user = seed_active_user(&db, PrimaryRole::User).await;

    let lookups = (0..CONCURRENT_QUERIES).map(|_| {
        let db = db.clone();
        async move { db.users().find_by_id(user.id).await }
    });

    for (i, r) in futures::future::join_all(lookups)
        .await
        .into_iter()
        .enumerate()
    {
        let found = r.unwrap_or_else(|e| panic!("concurrent lookup {i} failed: {e}"));
        assert!(found.is_some(), "concurrent lookup {i} found no user");
    }
}

/// Two test databases must not see each other, or tests contaminate one another
/// as soon as they run in parallel.
#[tokio::test]
async fn two_test_databases_are_isolated() {
    let first = connect_test_db().await;
    let second = connect_test_db().await;

    seed_active_user(&first, PrimaryRole::User).await;

    assert_eq!(first.dashboard().summary().await.unwrap().members_total, 1);
    assert_eq!(
        second.dashboard().summary().await.unwrap().members_total,
        0,
        "a second test database saw rows written by the first"
    );
}
