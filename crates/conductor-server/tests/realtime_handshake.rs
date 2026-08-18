mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::response::Response;
use conductor_server::RealtimeConfig;
use http_body_util::BodyExt;
use serde_json::Value;
use support::{test_app_with_realtime_config, TestApp};
use tower::ServiceExt;

#[tokio::test]
async fn handshake_capacity_is_checked_before_connection_authentication() {
    let app = test_app_with_realtime_config(load_config(8, 1)).await;
    let handshake = app
        .state
        .realtime
        .try_begin_handshake()
        .expect("reserve the only handshake permit");

    let rejected = realtime_request(&app, "evc_missing_realtime_credential").await;
    assert_capacity_response(
        rejected,
        StatusCode::SERVICE_UNAVAILABLE,
        "too many concurrent realtime handshakes",
    )
    .await;
    assert_eq!(app.state.realtime.active_connections(), 0);

    drop(handshake);
    let unauthorized = realtime_request(&app, "evc_missing_realtime_credential").await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(app.state.realtime.active_connections(), 0);
}

fn load_config(max_connections: usize, max_concurrent_handshakes: usize) -> RealtimeConfig {
    RealtimeConfig {
        max_connections,
        max_connections_per_secret: 4,
        max_concurrent_handshakes,
        broadcast_capacity: 512,
        heartbeat_seconds: 1,
    }
}

async fn realtime_request(app: &TestApp, token: &str) -> Response {
    app.router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/realtime/events")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::ACCEPT, "text/event-stream")
                .body(Body::empty())
                .expect("build realtime request"),
        )
        .await
        .expect("realtime response")
}

async fn assert_capacity_response(response: Response, status: StatusCode, message: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(
        response.headers().get(header::RETRY_AFTER),
        Some(&header::HeaderValue::from_static("5"))
    );
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect capacity response")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("capacity JSON");
    assert_eq!(json["error"], message);
    assert_eq!(json["retry_after_seconds"], 5);
}
