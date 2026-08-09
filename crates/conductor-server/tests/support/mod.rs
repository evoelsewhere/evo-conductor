//! Shared fixture for integration tests.
//!
//! Three facts about the application drive how this is built, and each of them
//! produces a confusing failure when ignored:
//!
//! 1. `Db::connect` builds a ten-connection pool. A plain `sqlite::memory:` URL
//!    gives every connection its own private database, and `migrate::run` fails
//!    partway through with `no such table: main.users`. Every test therefore
//!    gets a uniquely named shared-cache database.
//! 2. `AppState::new` reads the JWT secret from the `instance` row, which a
//!    fresh test database does not have. Without `set_jwt_secret` the extractor
//!    returns **428 SetupRequired**, not 401, and the test looks broken rather
//!    than unauthenticated.
//! 3. `AuthUser` admits only `UserStatus::Active`. `create_invited` leaves the
//!    user `Invited`, so seeding must promote it.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use conductor_auth::JwtService;
use conductor_domain::{CreateMemberRequest, PrimaryRole, User, UserStatus};
use conductor_server::{build_router, AppState, Config};
use http_body_util::BodyExt;
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;
use uuid::Uuid;

/// Argon2 hash of an arbitrary string. Tests never authenticate with a
/// password, and hashing per test would dominate the suite's runtime.
const PLACEHOLDER_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c3RlcDBzYWx0$0000000000000000000000000000000000000000000";

pub struct TestApp {
    pub router: Router,
    pub state: AppState,
    pub jwt: JwtService,
}

/// A running application backed by an empty, isolated database.
pub async fn test_app() -> TestApp {
    let database_url = format!(
        "sqlite:file:conductor_test_{}?mode=memory&cache=shared",
        Uuid::new_v4().simple()
    );

    let state = AppState::new(&database_url)
        .await
        .expect("connect test database");

    // Fact 2: without this every authenticated request returns 428.
    let secret = format!("test-secret-{}", Uuid::new_v4().simple());
    state.set_jwt_secret(secret.clone()).await;

    let config = Config {
        database_url,
        host: "127.0.0.1".into(),
        port: 0,
        web_dist: PathBuf::from("/nonexistent-web-dist-for-tests"),
    };

    TestApp {
        router: build_router(state.clone(), &config),
        state,
        jwt: JwtService::new(secret, 72),
    }
}

impl TestApp {
    /// An active member with the given role.
    pub async fn seed_user(&self, role: PrimaryRole) -> User {
        let req = CreateMemberRequest {
            email: format!("{}-{}@example.test", role.as_str(), Uuid::new_v4().simple()),
            display_name: format!("Test {}", role.as_str()),
            primary_role: role,
            sub_role_ids: vec![],
            tag_ids: vec![],
        };

        let user = self
            .state
            .db
            .users()
            .create_invited(&req, PLACEHOLDER_PASSWORD_HASH, Uuid::new_v4())
            .await
            .expect("create_invited");

        // Fact 3: AuthUser rejects Invited.
        self.state
            .db
            .users()
            .set_status(user.id, UserStatus::Active)
            .await
            .expect("activate seeded user")
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
            builder = builder.header("Authorization", format!("Bearer {token}"));
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
