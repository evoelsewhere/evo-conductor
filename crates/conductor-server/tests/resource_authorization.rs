mod support;

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use conductor_domain::{
    AuthorizationAction, CreateResourceRequest, PrimaryRole, ResourceKind, ResourceVisibility,
    SetupRequest,
};
use conductor_server::core::authorization::{
    AuthorizationDecisionObserver, AuthorizationEvent, AuthorizationResult, AuthorizationService,
    AuthorizationStage,
};
use conductor_storage::repos::DraftContent;
use serde_json::json;
use support::test_app_with_authorization;

#[derive(Default)]
struct RecordingObserver {
    events: Mutex<Vec<AuthorizationEvent>>,
}

impl AuthorizationDecisionObserver for RecordingObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        self.events
            .lock()
            .expect("observer lock")
            .push(event.clone());
    }
}

#[tokio::test]
async fn admin_nonowner_is_allowed_and_contributor_nonowner_is_opaque_at_target_stage() {
    let observer = Arc::new(RecordingObserver::default());
    let app = test_app_with_authorization(AuthorizationService::new(observer.clone())).await;
    let (project, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "target-resource-policy".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "target-admin@example.test".into(),
                admin_display_name: "Target Admin".into(),
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
    let resource = app
        .state
        .db
        .resources()
        .create(
            project.id,
            &CreateResourceRequest {
                kind: ResourceKind::Agent,
                slug: "target-owned-agent".into(),
                name: "Target owned agent".into(),
                description: None,
                version: "0.1.0".into(),
                visibility: ResourceVisibility::Private,
                payload: json!({}),
                changelog: None,
            },
            owner.id,
            &DraftContent {
                artifact_key:
                    "sha256/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .into(),
                sha256: "a".repeat(64),
                size: 1,
                metadata_payload: json!({}),
            },
        )
        .await
        .expect("create resource");
    let path = format!("/api/resources/{}", resource.id);

    let admin_token = app.token_for(&admin).await;
    let (status, body) = app
        .patch(
            &path,
            Some(&admin_token),
            json!({ "name": "Admin managed", "description": null, "visibility": null }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let peer_token = app.token_for(&peer).await;
    let (status, body) = app
        .patch(
            &path,
            Some(&peer_token),
            json!({ "name": "Peer overwrite", "description": null, "visibility": null }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    let foreign_project = uuid::Uuid::new_v4();
    let mut connection = app
        .state
        .db
        .pool()
        .acquire()
        .await
        .expect("acquire database connection");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .expect("disable foreign keys for corrupt target fixture");
    sqlx::query("UPDATE resources SET project_id = ? WHERE id = ?")
        .bind(foreign_project.to_string())
        .bind(resource.id.to_string())
        .execute(&mut *connection)
        .await
        .expect("move resource to foreign project");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("restore foreign-key enforcement");
    drop(connection);
    let (status, body) = app
        .patch(
            &path,
            Some(&admin_token),
            json!({ "name": "Cross-project overwrite", "description": null, "visibility": null }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = app.get("/api/resources", Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body
        .as_array()
        .expect("resource catalog array")
        .iter()
        .any(|item| item["id"] == resource.id.to_string()));
    sqlx::query("UPDATE resources SET project_id = ? WHERE id = ?")
        .bind(project.id.to_string())
        .bind(resource.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("restore resource project");

    sqlx::query("UPDATE resources SET status = 'published', visibility = 'shared' WHERE id = ?")
        .bind(resource.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("publish shared feedback target");
    let feedback_path = format!("{path}/feedback");
    let (status, body) = app
        .put(
            &feedback_path,
            Some(&peer_token),
            json!({"rating": 5, "comment": "Visible"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    sqlx::query("UPDATE resources SET visibility = 'private' WHERE id = ?")
        .bind(resource.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("make feedback target private");
    let (status, body) = app
        .put(
            &feedback_path,
            Some(&peer_token),
            json!({"rating": 4, "comment": "Invisible"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    sqlx::query("UPDATE resources SET kind = 'future_executable' WHERE id = ?")
        .bind(resource.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("corrupt persisted resource kind");
    let owner_token = app.token_for(&owner).await;
    let (status, body) = app
        .patch(
            &path,
            Some(&owner_token),
            json!({ "name": "Coerced agent", "description": null, "visibility": null }),
        )
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(!body.to_string().contains("future_executable"));
    let stored_name: String = sqlx::query_scalar("SELECT name FROM resources WHERE id = ?")
        .bind(resource.id.to_string())
        .fetch_one(app.state.db.pool())
        .await
        .expect("load unchanged resource");
    assert_eq!(stored_name, "Admin managed");

    let events = observer.events.lock().expect("observer lock");
    let target_events = events
        .iter()
        .filter(|event| {
            event.stage == AuthorizationStage::Target
                && event.action == AuthorizationAction::ResourceUpdate
        })
        .collect::<Vec<_>>();
    assert_eq!(target_events.len(), 3, "{target_events:#?}");
    assert!(target_events.iter().any(|event| {
        event.actor_id == admin.id
            && event.target_id == Some(resource.id)
            && event.authorization_result == AuthorizationResult::Allowed
    }));
    assert!(target_events.iter().any(|event| {
        event.actor_id == peer.id
            && event.target_id == Some(resource.id)
            && event.authorization_result == AuthorizationResult::Denied
            && event.reason_code == Some(conductor_domain::DecisionReason::DenyNotOwner)
    }));
    assert!(target_events.iter().any(|event| {
        event.actor_id == admin.id
            && event.project_id == Some(foreign_project)
            && event.authorization_result == AuthorizationResult::Denied
            && event.reason_code == Some(conductor_domain::DecisionReason::DenyCrossProject)
    }));
    let feedback_events = events
        .iter()
        .filter(|event| {
            event.stage == AuthorizationStage::Target
                && event.action == AuthorizationAction::ResourceFeedbackSubmit
        })
        .collect::<Vec<_>>();
    assert_eq!(feedback_events.len(), 2, "{feedback_events:#?}");
    assert!(feedback_events.iter().any(|event| {
        event.target_id == Some(resource.id)
            && event.authorization_result == AuthorizationResult::Allowed
    }));
    assert!(feedback_events.iter().any(|event| {
        event.target_id == Some(resource.id)
            && event.authorization_result == AuthorizationResult::Denied
            && event.reason_code == Some(conductor_domain::DecisionReason::DenyOutsideAudience)
    }));
}
