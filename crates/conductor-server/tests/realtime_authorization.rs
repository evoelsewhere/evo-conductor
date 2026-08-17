mod support;

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{PrimaryRole, SecretScope};
use http_body_util::BodyExt;
use support::test_app;
use tower::ServiceExt;

#[tokio::test]
async fn heartbeat_revalidates_a_revoked_secret_even_when_the_signal_was_missed() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let mut realtime = app.state.realtime.config();
    realtime.heartbeat_seconds = 1;
    app.state.realtime.update_config(realtime);

    let owner = app.seed_user(PrimaryRole::User).await;
    let raw_token = "evc_realtime_heartbeat_revalidation";
    let secret = app
        .state
        .db
        .secrets()
        .insert(
            owner.id,
            "realtime heartbeat",
            "evc_realtime",
            &hash_token(raw_token),
            &[SecretScope::SubscribeResources],
            None,
        )
        .await
        .expect("seed connection secret");

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/realtime/events")
                .header("Authorization", format!("Bearer {raw_token}"))
                .body(Body::empty())
                .expect("build realtime request"),
        )
        .await
        .expect("open realtime response");
    assert_eq!(response.status(), StatusCode::OK);

    // Mutate durable state directly so no local AccessRevoked signal can help
    // the stream. The next heartbeat must independently revalidate the token.
    sqlx::query("UPDATE connection_secrets SET revoked_at = ? WHERE id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(secret.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("revoke secret without realtime signal");

    let mut body = response.into_body();
    let mut received = String::new();
    tokio::time::timeout(Duration::from_secs(4), async {
        while let Some(frame) = body.frame().await {
            let frame = frame.expect("read realtime frame");
            if let Ok(bytes) = frame.into_data() {
                received.push_str(&String::from_utf8_lossy(&bytes));
                if received.contains("control.access_revoked") {
                    return;
                }
            }
        }
        panic!("realtime stream ended without an access-revoked event: {received}");
    })
    .await
    .unwrap_or_else(|_| panic!("heartbeat did not revoke stale access: {received}"));

    assert!(received.contains("access_no_longer_valid"), "{received}");
}
