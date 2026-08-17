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
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use conductor_auth::JwtService;
use conductor_domain::core::constants::auth::{AUTH_SCHEME_BEARER, DEFAULT_JWT_TTL_HOURS};
use conductor_domain::CreateMemberRequest;
use conductor_domain::{PrimaryRole, User};
use conductor_server::core::authorization::AuthorizationService;
use conductor_server::{build_router, AppState, Config, RealtimeConfig};
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
        .activate_invited_on_password_login(user.id)
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
    test_app_with_authorization(AuthorizationService::default()).await
}

pub async fn test_app_with_authorization(authorization: AuthorizationService) -> TestApp {
    let database_url = test_database_url();

    let mut state = AppState::new(&database_url, RealtimeConfig::default())
        .await
        .expect("connect test database");
    state.authorization = authorization;

    // Fact 1: without this every authenticated request returns 428.
    let secret = format!("{TEST_JWT_SECRET_PREFIX}{}", Uuid::new_v4().simple());
    state.set_jwt_secret(secret.clone()).await;

    let config = Config {
        database_url,
        host: TEST_BIND_HOST.into(),
        port: TEST_BIND_PORT,
        web_dist: PathBuf::from(UNUSED_WEB_DIST),
        realtime: RealtimeConfig::default(),
    };

    TestApp {
        router: build_router(state.clone(), &config),
        state,
        jwt: JwtService::new(secret, DEFAULT_JWT_TTL_HOURS),
    }
}

impl TestApp {
    /// Seed the singleton project identity required by project-scoped
    /// authorization without marking setup complete or creating an admin.
    ///
    /// Most authorization tests exercise protected routes directly, while the
    /// default fixture intentionally remains unconfigured for bootstrap tests.
    pub async fn seed_project_identity(&self) -> Uuid {
        let project_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO instance (
                id, project_name, display_name, bind_host, bind_port,
                setup_completed, jwt_secret, created_at, updated_at
            ) VALUES (?, 'Test project', 'Test project', '127.0.0.1', 0,
                      0, 'test-project-secret',
                      '2026-08-17T00:00:00Z', '2026-08-17T00:00:00Z')
            "#,
        )
        .bind(project_id.to_string())
        .execute(self.state.db.pool())
        .await
        .expect("seed project identity");
        project_id
    }

    pub async fn seed_user(&self, role: PrimaryRole) -> User {
        seed_active_user(&self.state.db, role).await
    }

    /// A bearer token for an existing member.
    ///
    /// The session version is read from the database rather than assumed:
    /// `set_status` increments it, which is how disabling a member invalidates
    /// their sessions, and the fixture activates every seeded user. A token
    /// issued with a stale version is rejected by `AuthUser` with 401.
    pub async fn token_for(&self, user: &User) -> String {
        let session_version = self
            .state
            .db
            .users()
            .session_version(user.id)
            .await
            .expect("read session version")
            .expect("seeded user exists");

        self.jwt
            .issue(user.id, &user.email, user.primary_role, session_version)
            .expect("issue token")
            .0
    }

    /// Seed a user of the given role and return their bearer token.
    pub async fn token_for_role(&self, role: PrimaryRole) -> String {
        let user = self.seed_user(role).await;
        self.token_for(&user).await
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

    pub async fn patch(&self, path: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.send(
            Request::builder().method("PATCH").uri(path),
            token,
            Some(body),
        )
        .await
    }

    pub async fn put(&self, path: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.send(
            Request::builder().method("PUT").uri(path),
            token,
            Some(body),
        )
        .await
    }

    pub async fn delete(
        &self,
        path: &str,
        token: Option<&str>,
        body: Value,
    ) -> (StatusCode, Value) {
        self.send(
            Request::builder().method("DELETE").uri(path),
            token,
            Some(body),
        )
        .await
    }

    pub async fn post_with_headers(
        &self,
        path: &str,
        token: Option<&str>,
        headers: HeaderMap,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method("POST").uri(path);
        for (name, value) in headers {
            if let Some(name) = name {
                builder = builder.header(name, value);
            }
        }
        self.send(builder, token, Some(body)).await
    }

    pub async fn post_bytes(
        &self,
        path: &str,
        token: Option<&str>,
        content_type: &str,
        body: Vec<u8>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
            .header("Content-Type", content_type);
        if let Some(token) = token {
            builder = builder.header("Authorization", format!("{AUTH_SCHEME_BEARER}{token}"));
        }
        self.send_request(builder.body(Body::from(body)).expect("build request"))
            .await
    }

    pub async fn put_bytes(
        &self,
        path: &str,
        token: Option<&str>,
        content_type: &str,
        body: Vec<u8>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method("PUT")
            .uri(path)
            .header("Content-Type", content_type);
        if let Some(token) = token {
            builder = builder.header("Authorization", format!("{AUTH_SCHEME_BEARER}{token}"));
        }
        self.send_request(builder.body(Body::from(body)).expect("build request"))
            .await
    }

    pub async fn get_bytes(&self, path: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
        self.get_bytes_with_headers(path, None, HeaderMap::new())
            .await
    }

    pub async fn get_bytes_with_headers(
        &self,
        path: &str,
        token: Option<&str>,
        headers: HeaderMap,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut builder = Request::builder().method("GET").uri(path);
        if let Some(token) = token {
            builder = builder.header("Authorization", format!("{AUTH_SCHEME_BEARER}{token}"));
        }
        for (name, value) in headers {
            if let Some(name) = name {
                builder = builder.header(name, value);
            }
        }
        let response = self
            .router
            .clone()
            .oneshot(builder.body(Body::empty()).expect("build request"))
            .await
            .expect("router response");
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes()
            .to_vec();
        (status, headers, bytes)
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

        self.send_request(request).await
    }

    async fn send_request(&self, request: Request<Body>) -> (StatusCode, Value) {
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
