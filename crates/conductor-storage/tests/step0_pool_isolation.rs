//! Step 0 of TSK-020-01: prove that an in-memory SQLite URL survives the
//! ten-connection pool that `Db::connect` hardcodes.
//!
//! `Db::connect` builds the pool with `max_connections(10)`. Plain
//! `sqlite::memory:` gives every connection its own private empty database, so
//! migrations run on one connection and later queries can land on another that
//! has no tables. These tests decide whether the shared-cache URL avoids that,
//! before any of the real fixture is written on top of it.

use conductor_domain::{CreateMemberRequest, PrimaryRole};
use conductor_storage::Db;
use sqlx::Row;
use uuid::Uuid;

fn shared_cache_url() -> String {
    format!(
        "sqlite:file:conductor_test_{}?mode=memory&cache=shared",
        Uuid::new_v4().simple()
    )
}

async fn seed_one(db: &Db) -> Uuid {
    let req = CreateMemberRequest {
        email: "step0@example.test".into(),
        display_name: "Step Zero".into(),
        primary_role: PrimaryRole::User,
        sub_role_ids: vec![],
        tag_ids: vec![],
    };
    db.users()
        .create_invited(&req, "not-a-real-hash", Uuid::new_v4())
        .await
        .expect("create_invited")
        .id
}

/// The control: record what plain `sqlite::memory:` actually does with this
/// pool, so the reason for the shared-cache URL is evidence rather than
/// folklore. If this ever starts behaving, the design note should be revisited.
#[tokio::test]
async fn plain_memory_url_is_unusable() {
    match Db::connect("sqlite::memory:").await {
        Err(e) => {
            println!("plain sqlite::memory: rejected at connect time: {e}");
        }
        Ok(db) => {
            // Hold two connections at once so the pool must open a second one.
            let mut a = db.pool().acquire().await.expect("acquire a");
            let mut b = db.pool().acquire().await.expect("acquire b");
            let on_a = sqlx::query("SELECT COUNT(*) AS n FROM users")
                .fetch_one(&mut *a)
                .await;
            let on_b = sqlx::query("SELECT COUNT(*) AS n FROM users")
                .fetch_one(&mut *b)
                .await;
            println!(
                "plain sqlite::memory: connected; first connection ok={}, second ok={}",
                on_a.is_ok(),
                on_b.is_ok()
            );
            assert!(
                on_a.is_err() || on_b.is_err(),
                "plain sqlite::memory: now works across pooled connections — \
                 revisit DES-020 section 2, the shared-cache workaround may be unnecessary"
            );
        }
    }
}

/// The decision: a named shared-cache database must be visible from every
/// connection the pool hands out.
#[tokio::test]
async fn shared_cache_url_is_visible_from_every_pooled_connection() {
    let db = Db::connect(&shared_cache_url())
        .await
        .expect("connect with shared-cache in-memory URL");

    let user_id = seed_one(&db).await;

    // Force several connections open simultaneously and query on each.
    let mut held = Vec::new();
    for _ in 0..5 {
        held.push(db.pool().acquire().await.expect("acquire"));
    }
    for (i, conn) in held.iter_mut().enumerate() {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM users")
            .fetch_one(&mut **conn)
            .await
            .unwrap_or_else(|e| panic!("connection {i} could not see the users table: {e}"));
        let n: i64 = row.get("n");
        assert_eq!(n, 1, "connection {i} saw {n} users, expected 1");
    }
    drop(held);

    // And through two different repositories, which is how real tests will use it.
    let found = db.users().find_by_id(user_id).await.expect("find_by_id");
    assert!(
        found.is_some(),
        "user written by one repo is unreadable by another"
    );

    let summary = db.dashboard().summary().await.expect("dashboard summary");
    assert_eq!(summary.members_total, 1);
}

/// Concurrency is the case that breaks first in practice: many futures in
/// flight force the pool to spread work across connections.
#[tokio::test]
async fn shared_cache_url_survives_concurrent_queries() {
    let db = Db::connect(&shared_cache_url()).await.expect("connect");
    let user_id = seed_one(&db).await;

    let lookups = (0..25).map(|_| {
        let db = db.clone();
        async move { db.users().find_by_id(user_id).await }
    });

    let results = futures::future::join_all(lookups).await;
    for (i, r) in results.into_iter().enumerate() {
        let user = r.unwrap_or_else(|e| panic!("concurrent lookup {i} failed: {e}"));
        assert!(user.is_some(), "concurrent lookup {i} found no user");
    }
}

/// Two test databases must not see each other, or tests will contaminate one
/// another as soon as they run in parallel.
#[tokio::test]
async fn two_test_databases_are_isolated() {
    let first = Db::connect(&shared_cache_url())
        .await
        .expect("connect first");
    let second = Db::connect(&shared_cache_url())
        .await
        .expect("connect second");

    seed_one(&first).await;

    assert_eq!(first.dashboard().summary().await.unwrap().members_total, 1);
    assert_eq!(
        second.dashboard().summary().await.unwrap().members_total,
        0,
        "a second test database saw rows written by the first"
    );
}
