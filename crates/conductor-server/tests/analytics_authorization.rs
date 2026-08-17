mod support;

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use conductor_domain::{
    AuthorizationAction, DecisionReason, PermissionKey, PrimaryRole, SetupRequest,
};
use conductor_server::core::authorization::{
    AuthorizationDecisionObserver, AuthorizationEvent, AuthorizationResult, AuthorizationService,
    AuthorizationStage,
};
use serde_json::{json, Value};
use support::test_app_with_authorization;
use uuid::Uuid;

#[derive(Default)]
struct RecordingObserver(Mutex<Vec<AuthorizationEvent>>);

impl AuthorizationDecisionObserver for RecordingObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        self.0.lock().expect("observer lock").push(event.clone());
    }
}

fn view_body(name: &str, visibility: &str) -> Value {
    json!({
        "name": name,
        "description": null,
        "visibility": visibility,
        "definition": {
            "schema_version": 1,
            "preset": "executive",
            "density": "comfortable",
            "query": { "date_range": "last_30_days", "comparison": null },
            "widgets": [{
                "id": "request-volume",
                "title": "Request volume",
                "visualization": "area",
                "metric": "requests",
                "group_by": "time",
                "size": "full",
                "limit": 10,
                "show_legend": false
            }]
        }
    })
}

fn identifying_view_body(name: &str, member_id: Uuid, installation_id: Uuid) -> Value {
    let mut body = view_body(name, "shared");
    body["definition"]["query"]["member_id"] = json!(member_id);
    body["definition"]["query"]["installation_id"] = json!(installation_id);
    body
}

#[tokio::test]
async fn analytics_view_read_uses_visibility_audience_without_granting_peer_mutation() {
    let observer = Arc::new(RecordingObserver::default());
    let app = test_app_with_authorization(AuthorizationService::new(observer.clone())).await;
    app.state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "analytics-target-policy".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "analytics-target-admin@example.test".into(),
                admin_display_name: "Analytics Target Admin".into(),
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
    let owner_token = app.token_for(&owner).await;
    let peer_token = app.token_for(&peer).await;

    let (private_status, private) = app
        .post(
            "/api/analytics/views",
            Some(&owner_token),
            view_body("Private target", "private"),
        )
        .await;
    assert_eq!(private_status, StatusCode::OK, "{private}");
    let (shared_status, shared) = app
        .post(
            "/api/analytics/views",
            Some(&owner_token),
            view_body("Shared target", "shared"),
        )
        .await;
    assert_eq!(shared_status, StatusCode::OK, "{shared}");
    let private_id = private["id"].as_str().expect("private view id");
    let shared_id = shared["id"].as_str().expect("shared view id");

    assert_eq!(
        app.get(
            &format!("/api/analytics/views/{private_id}"),
            Some(&peer_token),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get(
            &format!("/api/analytics/views/{shared_id}"),
            Some(&peer_token),
        )
        .await
        .0,
        StatusCode::OK
    );

    let events = observer.0.lock().expect("observer lock");
    let target_events = events
        .iter()
        .filter(|event| {
            event.stage == AuthorizationStage::Target
                && event.action == AuthorizationAction::AnalyticsViewRead
        })
        .collect::<Vec<_>>();
    assert_eq!(target_events.len(), 2, "{target_events:#?}");
    assert!(target_events.iter().any(|event| {
        event.target_id.map(|id| id.to_string()).as_deref() == Some(private_id)
            && event.authorization_result == AuthorizationResult::Denied
            && event.reason_code == Some(DecisionReason::DenyOutsideAudience)
    }));
    assert!(target_events.iter().any(|event| {
        event.target_id.map(|id| id.to_string()).as_deref() == Some(shared_id)
            && event.authorization_result == AuthorizationResult::Allowed
            && event.resolved_permission == Some(PermissionKey::AnalyticsViewRead)
    }));
}

#[tokio::test]
async fn shared_identifying_view_is_hidden_from_non_owner_contributor() {
    let app = test_app_with_authorization(AuthorizationService::default()).await;
    app.state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "analytics-shared-view-privacy".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4701,
                public_url: None,
                admin_email: "analytics-privacy-admin@example.test".into(),
                admin_display_name: "Analytics Privacy Admin".into(),
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
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let owner_token = app.token_for(&owner).await;
    let peer_token = app.token_for(&peer).await;
    let admin_token = app.token_for(&admin).await;
    let installation_id = Uuid::new_v4();

    let (safe_status, safe) = app
        .post(
            "/api/analytics/views",
            Some(&owner_token),
            view_body("Shared aggregate", "shared"),
        )
        .await;
    assert_eq!(safe_status, StatusCode::OK, "{safe}");
    let safe_id = safe["id"].as_str().expect("safe shared view id");

    let (identifying_status, identifying) = app
        .post(
            "/api/analytics/views",
            Some(&owner_token),
            identifying_view_body("Shared member detail", owner.id, installation_id),
        )
        .await;
    assert_eq!(identifying_status, StatusCode::OK, "{identifying}");
    let identifying_id = identifying["id"]
        .as_str()
        .expect("identifying shared view id");

    let (peer_list_status, peer_list) = app.get("/api/analytics/views", Some(&peer_token)).await;
    assert_eq!(peer_list_status, StatusCode::OK, "{peer_list}");
    let peer_views = peer_list.as_array().expect("analytics view list");
    let peer_view_ids = peer_views
        .iter()
        .filter_map(|view| view["id"].as_str())
        .collect::<Vec<_>>();
    assert!(peer_view_ids.contains(&safe_id));
    assert!(!peer_view_ids.contains(&identifying_id));
    assert!(peer_views.iter().all(|view| {
        view["definition"]["query"]["member_id"].is_null()
            && view["definition"]["query"]["installation_id"].is_null()
    }));

    let (peer_get_status, peer_get) = app
        .get(
            &format!("/api/analytics/views/{identifying_id}"),
            Some(&peer_token),
        )
        .await;
    assert_eq!(peer_get_status, StatusCode::NOT_FOUND, "{peer_get}");
    let peer_get_json = peer_get.to_string();
    assert!(!peer_get_json.contains(&owner.id.to_string()));
    assert!(!peer_get_json.contains(&installation_id.to_string()));

    for (label, token) in [("owner", &owner_token), ("admin", &admin_token)] {
        let (status, body) = app
            .get(
                &format!("/api/analytics/views/{identifying_id}"),
                Some(token),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{label}: {body}");
        assert_eq!(body["definition"]["query"]["member_id"], json!(owner.id));
        assert_eq!(
            body["definition"]["query"]["installation_id"],
            json!(installation_id)
        );
    }
}
