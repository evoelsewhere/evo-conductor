mod support;

use axum::http::StatusCode;
use conductor_domain::{PrimaryRole, SetupRequest};
use serde_json::{json, Value};
use support::test_app;

fn view_body(name: &str, visibility: &str) -> Value {
    json!({
        "name": name,
        "description": "Operations dashboard",
        "visibility": visibility,
        "definition": {
            "schema_version": 1,
            "preset": "executive",
            "density": "comfortable",
            "query": {
                "date_range": "last_30_days",
                "comparison": "previous_period"
            },
            "widgets": [
                {
                    "id": "request-volume",
                    "title": "Request volume",
                    "visualization": "area",
                    "metric": "requests",
                    "group_by": "time",
                    "size": "full",
                    "limit": 10,
                    "show_legend": false
                },
                {
                    "id": "success-rate",
                    "title": "Success rate",
                    "visualization": "kpi",
                    "metric": "success_rate",
                    "group_by": null,
                    "size": "one_third",
                    "limit": 10,
                    "show_legend": false
                }
            ]
        }
    })
}

#[tokio::test]
async fn saved_views_enforce_visibility_ownership_and_optimistic_revision() {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "analytics-views-api".into(),
                display_name: Some("Analytics views API".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "analytics-admin@example.test".into(),
                admin_display_name: "Analytics Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let owner = app.seed_user(PrimaryRole::Contribute).await;
    let peer = app.seed_user(PrimaryRole::Contribute).await;
    let plain_user = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;
    let owner_token = app.token_for(&owner).await;
    let peer_token = app.token_for(&peer).await;
    let plain_token = app.token_for(&plain_user).await;

    let (status, _) = app.get("/api/analytics/views", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.get("/api/analytics/views", Some(&plain_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, created) = app
        .post(
            "/api/analytics/views",
            Some(&owner_token),
            view_body("Operations", "private"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["revision"], 1);
    assert_eq!(created["owner_user_id"], owner.id.to_string());
    assert_eq!(created["definition"]["preset"], "executive");
    let id = created["id"].as_str().expect("view id");

    let (status, owner_list) = app.get("/api/analytics/views", Some(&owner_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(owner_list.as_array().map(Vec::len), Some(1));
    let (status, peer_list) = app.get("/api/analytics/views", Some(&peer_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(peer_list.as_array().map(Vec::len), Some(0));
    let (status, _) = app
        .get(&format!("/api/analytics/views/{id}"), Some(&peer_token))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = app
        .get(&format!("/api/analytics/views/{id}"), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK);

    let mut shared = view_body("Operations", "shared");
    shared["revision"] = json!(1);
    let (status, updated) = app
        .put(
            &format!("/api/analytics/views/{id}"),
            Some(&owner_token),
            shared.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["revision"], 2);

    let (status, _) = app
        .get(&format!("/api/analytics/views/{id}"), Some(&peer_token))
        .await;
    assert_eq!(status, StatusCode::OK);
    shared["revision"] = json!(2);
    let (status, _) = app
        .put(
            &format!("/api/analytics/views/{id}"),
            Some(&peer_token),
            shared.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    shared["revision"] = json!(1);
    let (status, conflict) = app
        .put(
            &format!("/api/analytics/views/{id}"),
            Some(&owner_token),
            shared.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");

    let (status, _) = app
        .delete(
            &format!("/api/analytics/views/{id}?revision=1"),
            Some(&owner_token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, deleted) = app
        .delete(
            &format!("/api/analytics/views/{id}?revision=2"),
            Some(&owner_token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert_eq!(deleted["deleted"], true);
}

#[tokio::test]
async fn saved_views_reject_unknown_query_language_and_invalid_widgets() {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "analytics-validation-api".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "analytics-validation@example.test".into(),
                admin_display_name: "Analytics Validation".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let token = app.token_for(&admin).await;

    let mut raw_query = view_body("Unsafe", "private");
    raw_query["definition"]["query"] = json!({
        "date_range": "last_30_days",
        "sql": "select * from telemetry_events"
    });
    let (status, _) = app
        .post("/api/analytics/views", Some(&token), raw_query)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let mut invalid_widget = view_body("Invalid widget", "private");
    invalid_widget["definition"]["widgets"][0]["group_by"] = Value::Null;
    let (status, body) = app
        .post("/api/analytics/views", Some(&token), invalid_widget)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "chart and table widgets require group_by");
}
