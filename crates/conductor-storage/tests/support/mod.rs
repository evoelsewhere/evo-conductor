//! Test-only helpers for the storage suite.
//!
//! Production constants live in `conductor_storage::core`; only values that
//! exist solely for tests belong here.

#![allow(dead_code)]

use conductor_domain::{CreateMemberRequest, PrimaryRole, User, UserStatus};
use conductor_storage::core::url::sqlite_shared_memory_url;
use conductor_storage::Db;
use uuid::Uuid;

/// Prefix of every generated test database name, so a leak is obviously ours.
pub const TEST_DB_NAME_PREFIX: &str = "conductor_test_";

/// Domain for seeded addresses. `.test` is reserved by RFC 2606.
pub const TEST_EMAIL_DOMAIN: &str = "example.test";

/// Argon2-shaped placeholder. These tests never verify a password, and hashing
/// per seeded user would dominate the suite's runtime.
pub const PLACEHOLDER_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c3RlcDBzYWx0$0000000000000000000000000000000000000000000";

pub fn test_database_url() -> String {
    sqlite_shared_memory_url(&format!("{TEST_DB_NAME_PREFIX}{}", Uuid::new_v4().simple()))
}

pub async fn connect_test_db() -> Db {
    Db::connect(&test_database_url())
        .await
        .expect("connect test database")
}

/// Seed a member and promote it to `Active`; `create_invited` leaves it
/// `Invited`, which the server's extractor rejects.
pub async fn seed_active_user(db: &Db, role: PrimaryRole) -> User {
    let req = CreateMemberRequest {
        email: format!(
            "{}-{}@{TEST_EMAIL_DOMAIN}",
            role.as_str(),
            Uuid::new_v4().simple()
        ),
        display_name: format!("Test {}", role.as_str()),
        primary_role: role,
        sub_role_ids: vec![],
        tag_ids: vec![],
    };

    let user = db
        .users()
        .create_invited(&req, PLACEHOLDER_PASSWORD_HASH, Uuid::new_v4())
        .await
        .expect("create_invited");

    db.users()
        .set_status(user.id, UserStatus::Active)
        .await
        .expect("activate seeded user")
}
