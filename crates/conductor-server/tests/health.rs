//! Smoke tests proving the fixture builds a working application.

mod support;

use axum::http::StatusCode;
use conductor_domain::PrimaryRole;
use conductor_server::core::routes::{absolute, public, session};
use support::test_app;

/// The dialect the fixture is expected to report.
const EXPECTED_DIALECT: &str = "sqlite";

#[tokio::test]
async fn health_reports_the_active_dialect() {
    let app = test_app().await;

    let (status, body) = app.get(&absolute(public::HEALTH), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "evo-conductor");
    assert_eq!(body["database"], EXPECTED_DIALECT);
}

#[tokio::test]
async fn seeded_user_is_active_and_gets_a_usable_token() {
    let app = test_app().await;

    let user = app.seed_user(PrimaryRole::Admin).await;
    assert_eq!(user.primary_role, PrimaryRole::Admin);

    let token = app.token_for(&user);
    let claims = app.jwt.verify(&token).expect("token verifies");
    assert_eq!(claims.sub, user.id.to_string());
    assert_eq!(claims.role, PrimaryRole::Admin.as_str());

    // The point of activating the seeded user: an authenticated route accepts
    // it. A 428 here means the fixture forgot set_jwt_secret; a 403 means the
    // user was left Invited.
    let (status, _) = app.get(&absolute(session::AUTH_ME), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "seeded user should authenticate");
}

#[tokio::test]
async fn each_test_app_gets_its_own_database() {
    let a = test_app().await;
    let b = test_app().await;

    a.seed_user(PrimaryRole::User).await;

    let (_, body_a) = a.get(&absolute(public::SETUP_STATUS), None).await;
    let (_, body_b) = b.get(&absolute(public::SETUP_STATUS), None).await;
    assert_eq!(body_a["configured"], false);
    assert_eq!(body_b["configured"], false);

    let count_a = a.state.db.users().list().await.unwrap().len();
    let count_b = b.state.db.users().list().await.unwrap().len();
    assert_eq!(count_a, 1);
    assert_eq!(count_b, 0, "test databases are leaking into each other");
}

#[tokio::test]
async fn unauthenticated_request_is_rejected_with_401_not_428() {
    let app = test_app().await;

    let (status, _) = app.get(&absolute(session::AUTH_ME), None).await;

    // 428 here would mean the fixture failed to install a JWT secret, which is
    // the failure mode this assertion exists to catch.
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "expected 401; a 428 means the fixture did not set a JWT secret"
    );
}
