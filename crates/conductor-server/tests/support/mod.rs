//! HTTP fixture for the server suite.
//!
//! Only test-only values live here. Anything production also depends on comes
//! from `conductor_server::core` or `conductor_domain::core`, so a test can
//! never drift from the thing it is asserting against.
//!
//! Two application facts drive this fixture, and each produces a misleading
//! failure when ignored:
//!
//! 1. `AppState::new` reads the JWT secret from the `instance` row, which a
//!    fresh test database does not have. Without [`AppState::set_jwt_secret`]
//!    the extractor returns **428 SetupRequired**, not 401, and the test looks
//!    broken rather than unauthenticated.
//! 2. `AuthUser` admits only `UserStatus::Active`, while `create_invited` leaves
//!    the user `Invited`; [`seed_active_user`] promotes it.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use conductor_auth::JwtService;
use conductor_domain::core::constants::auth::{AUTH_SCHEME_BEARER, DEFAULT_JWT_TTL_HOURS};
use conductor_domain::{CreateMemberRequest, UserStatus};
use conductor_domain::{PrimaryRole, User};
use conductor_server::{build_router, AppState, Config};
use conductor_storage::core::url::sqlite_shared_memory_url;
use http_body_util::BodyExt;
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;
use uuid::Uuid;

/// Test-only values. Anything production also cares about lives in
/// `conductor_server::core` or `conductor_domain::core`, not here.
const TEST_DB_NAME_PREFIX: &str = "conductor_test_";
const TEST_EMAIL_DOMAIN: &str = "example.test";
const TEST_JWT_SECRET_PREFIX: &str = "test-secret-";
const TEST_BIND_HOST: &str = "127.0.0.1";
/// Zero: the fixture never binds a socket, it calls the router directly.
const TEST_BIND_PORT: u16 = 0;
/// The router serves the console from disk as a fallback; tests exercise the
/// API only, so this deliberately points nowhere.
const UNUSED_WEB_DIST: &str = "/nonexistent-web-dist-for-tests";
/// Argon2-shaped placeholder; tests never verify a password.
const PLACEHOLDER_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c3RlcDBzYWx0$0000000000000000000000000000000000000000000";

fn test_database_url() -> String {
    sqlite_shared_memory_url(&format!("{TEST_DB_NAME_PREFIX}{}", Uuid::new_v4().simple()))
}

async fn seed_active_user(db: &conductor_storage::Db, role: PrimaryRole) -> User {
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

pub struct TestApp {
    pub router: Router,
    pub state: AppState,
    pub jwt: JwtService,
}

/// A running application backed by an empty, isolated database.
pub async fn test_app() -> TestApp {
    let database_url = test_database_url();

    let state = AppState::new(&database_url)
        .await
        .expect("connect test database");

    // Fact 1: without this every authenticated request returns 428.
    let secret = format!("{TEST_JWT_SECRET_PREFIX}{}", Uuid::new_v4().simple());
    state.set_jwt_secret(secret.clone()).await;

    let config = Config {
        database_url,
        host: TEST_BIND_HOST.into(),
        port: TEST_BIND_PORT,
        web_dist: PathBuf::from(UNUSED_WEB_DIST),
    };

    TestApp {
        router: build_router(state.clone(), &config),
        state,
        jwt: JwtService::new(secret, DEFAULT_JWT_TTL_HOURS),
    }
}

impl TestApp {
    pub async fn seed_user(&self, role: PrimaryRole) -> User {
        seed_active_user(&self.state.db, role).await
    }

    pub fn token_for(&self, user: &User) -> String {
        self.jwt
            .issue(user.id, &user.email, user.primary_role)
            .expect("issue token")
            .0
    }

    /// Seed a user of the given role and return their bearer token.
    pub async fn token_for_role(&self, role: PrimaryRole) -> String {
        let user = self.seed_user(role).await;
        self.token_for(&user)
    }

    pub async fn get(&self, path: &str, token: Option<&str>) -> (StatusCode, Value) {
        self.send(Request::builder().method("GET").uri(path), token, None)
            .await
    }

    pub async fn post(&self, path: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.send(
            Request::builder().method("POST").uri(path),
            token,
            Some(body),
        )
        .await
    }

    async fn send(
        &self,
        mut builder: axum::http::request::Builder,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        if let Some(token) = token {
            builder = builder.header("Authorization", format!("{AUTH_SCHEME_BEARER}{token}"));
        }

        let request = match body {
            Some(json) => builder
                .header("Content-Type", "application/json")
                .body(Body::from(json.to_string()))
                .expect("build request"),
            None => builder.body(Body::empty()).expect("build request"),
        };

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router response");

        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();

        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };

        (status, json)
    }
}
