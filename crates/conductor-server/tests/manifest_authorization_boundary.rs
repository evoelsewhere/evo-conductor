//! Exhaustive negative HTTP proof for every protected action in the route manifest.
//!
//! These tests deliberately send a malformed JSON body. Authentication and the
//! role/scope pre-checks must reject the request before any handler extractor can
//! parse that body. Success-path handler fixtures are intentionally out of scope.

mod support;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::core::constants::auth::AUTH_SCHEME_BEARER;
use conductor_domain::{role_has_permission, PrimaryRole, SecretScope};
use conductor_server::http::authorization::{route_manifest, RouteAuthentication, RouteSpec};
use http_body_util::BodyExt;
use serde_json::Value;
use support::{test_app, TestApp};
use tower::ServiceExt;

const EXPECTED_BROWSER_ACTIONS: usize = 77;
const EXPECTED_CONNECTION_ACTIONS: usize = 11;
const EXPECTED_DENIED_ROLE_ACTION_CASES: usize = 81;

const PATH_ID: &str = "00000000-0000-0000-0000-000000000001";
const PATH_VERSION_ID: &str = "00000000-0000-0000-0000-000000000002";
const PATH_SECRET_ID: &str = "00000000-0000-0000-0000-000000000003";
const PATH_REQUEST_ID: &str = "00000000-0000-0000-0000-000000000004";
const PATH_ENTITY_ID: &str = "00000000-0000-0000-0000-000000000005";

const SUBSCRIBE_TOKEN: &str = "evc_manifest_subscribe_boundary_proof";
const TELEMETRY_TOKEN: &str = "evc_manifest_telemetry_boundary_proof";
const INVENTORY_TOKEN: &str = "evc_manifest_inventory_boundary_proof";

struct BoundaryResponse {
    status: StatusCode,
    body: Value,
}

fn concrete_path(template: &str) -> String {
    let route_path = template
        .replace("{version_id}", PATH_VERSION_ID)
        .replace("{secret_id}", PATH_SECRET_ID)
        .replace("{request_id}", PATH_REQUEST_ID)
        .replace("{entity_type}", "member")
        .replace("{entity_id}", PATH_ENTITY_ID)
        .replace("{kind}", "agent")
        .replace("{*path}", "manifest.json")
        .replace("{id}", PATH_ID);

    assert!(
        !route_path.contains('{') && !route_path.contains('}'),
        "manifest path has an unhandled parameter: {template}"
    );
    format!("/api{route_path}")
}

async fn send_manifest_request(
    app: &TestApp,
    route: &RouteSpec,
    credential: Option<&str>,
) -> BoundaryResponse {
    let method = Method::from_bytes(route.method.as_str().as_bytes()).expect("manifest method");
    let mut builder = Request::builder()
        .method(method)
        .uri(concrete_path(route.path))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(credential) = credential {
        builder = builder.header(
            header::AUTHORIZATION,
            format!("{AUTH_SCHEME_BEARER}{credential}"),
        );
    }

    let response = app
        .router
        .clone()
        .oneshot(
            builder
                .body(Body::from("{ malformed boundary proof"))
                .expect("build manifest request"),
        )
        .await
        .expect("manifest route response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect manifest response")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

    BoundaryResponse { status, body }
}

fn assert_boundary_error(
    route: &RouteSpec,
    response: &BoundaryResponse,
    expected_status: StatusCode,
    expected_error_code: &str,
    case: &str,
) {
    let route_label = format!(
        "{} {} ({}) [{case}]",
        route.method.as_str(),
        route.path,
        route.route_id
    );
    assert_eq!(response.status, expected_status, "status for {route_label}");
    assert_eq!(
        response.body.get("error_code").and_then(Value::as_str),
        Some(expected_error_code),
        "error_code for {route_label}; body={}",
        response.body
    );
}

async fn seed_connection_token(
    app: &TestApp,
    owner_user_id: uuid::Uuid,
    raw_token: &str,
    scope: SecretScope,
) {
    let token_hash = hash_token(raw_token);
    app.state
        .db
        .secrets()
        .insert(
            owner_user_id,
            &format!("{} boundary proof", scope.as_str()),
            scope.as_str(),
            &token_hash,
            &[scope],
            None,
        )
        .await
        .expect("seed connection token");
}

fn token_with_wrong_scope(required_scope: SecretScope) -> &'static str {
    match required_scope {
        SecretScope::SubscribeResources => TELEMETRY_TOKEN,
        SecretScope::ReportTelemetry => INVENTORY_TOKEN,
        SecretScope::SyncInventory => SUBSCRIBE_TOKEN,
    }
}

#[tokio::test]
async fn every_browser_manifest_action_fails_closed_at_the_axum_boundary() {
    let app = test_app().await;
    let mut browser_tokens = Vec::new();
    for role in PrimaryRole::ALL {
        let user = app.seed_user(role).await;
        browser_tokens.push((role, app.token_for(&user).await));
    }

    let connection_owner = app.seed_user(PrimaryRole::User).await;
    seed_connection_token(
        &app,
        connection_owner.id,
        SUBSCRIBE_TOKEN,
        SecretScope::SubscribeResources,
    )
    .await;

    let manifest = route_manifest();
    let mut browser_actions = 0;
    let mut denied_role_cases = 0;

    for route in &manifest.routes {
        let RouteAuthentication::Browser(policy) = &route.authentication else {
            continue;
        };
        browser_actions += 1;

        let missing = send_manifest_request(&app, route, None).await;
        assert_boundary_error(
            route,
            &missing,
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing credential",
        );

        let connection_credential = send_manifest_request(&app, route, Some(SUBSCRIBE_TOKEN)).await;
        assert_boundary_error(
            route,
            &connection_credential,
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "valid evc_ credential on browser route",
        );

        for (role, browser_token) in &browser_tokens {
            let has_eligible_grant = policy
                .alternatives
                .iter()
                .any(|alternative| role_has_permission(*role, alternative.permission));
            if has_eligible_grant {
                continue;
            }

            denied_role_cases += 1;
            let denied = send_manifest_request(&app, route, Some(browser_token)).await;
            assert_boundary_error(
                route,
                &denied,
                StatusCode::FORBIDDEN,
                "permission_denied",
                &format!("{} has no eligible permission grant", role.as_str()),
            );
        }
    }

    assert_eq!(browser_actions, EXPECTED_BROWSER_ACTIONS);
    assert_eq!(denied_role_cases, EXPECTED_DENIED_ROLE_ACTION_CASES);
}

#[tokio::test]
async fn every_connection_manifest_action_rejects_browser_and_wrong_scope_credentials() {
    let app = test_app().await;
    let browser_token = app.token_for_role(PrimaryRole::Admin).await;
    let connection_owner = app.seed_user(PrimaryRole::User).await;
    for (raw_token, scope) in [
        (SUBSCRIBE_TOKEN, SecretScope::SubscribeResources),
        (TELEMETRY_TOKEN, SecretScope::ReportTelemetry),
        (INVENTORY_TOKEN, SecretScope::SyncInventory),
    ] {
        seed_connection_token(&app, connection_owner.id, raw_token, scope).await;
    }

    let manifest = route_manifest();
    let mut connection_actions = 0;

    for route in &manifest.routes {
        let RouteAuthentication::Connection(policy) = &route.authentication else {
            continue;
        };
        connection_actions += 1;

        let browser_credential = send_manifest_request(&app, route, Some(&browser_token)).await;
        assert_boundary_error(
            route,
            &browser_credential,
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "browser JWT on connection route",
        );

        let wrong_scope = send_manifest_request(
            &app,
            route,
            Some(token_with_wrong_scope(policy.required_scope)),
        )
        .await;
        assert_boundary_error(
            route,
            &wrong_scope,
            StatusCode::FORBIDDEN,
            "scope_denied",
            "valid connection token with wrong route scope",
        );
    }

    assert_eq!(connection_actions, EXPECTED_CONNECTION_ACTIONS);
}
