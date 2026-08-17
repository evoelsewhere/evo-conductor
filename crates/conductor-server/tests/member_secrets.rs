mod support;

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use conductor_domain::{AuthorizationAction, PrimaryRole, SecretScope};
use conductor_server::core::authorization::{
    AuthorizationDecisionObserver, AuthorizationEvent, AuthorizationService, AuthorizationStage,
};
use serde_json::json;
use support::{test_app, test_app_with_authorization};
use uuid::Uuid;

#[derive(Default)]
struct RecordingObserver(Mutex<Vec<AuthorizationEvent>>);

impl AuthorizationDecisionObserver for RecordingObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        self.0.lock().expect("observer lock").push(event.clone());
    }
}

#[tokio::test]
async fn members_self_issue_and_admin_can_manage_metadata_without_minting_on_behalf() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;
    let member_token = app.token_for(&member).await;

    let (status, denied) = app
        .post(
            &format!("/api/members/{}/secrets", member.id),
            Some(&admin_token),
            json!({
                "name": "Member desktop",
                "scopes": [
                    SecretScope::SubscribeResources,
                    SecretScope::ReportTelemetry,
                    SecretScope::SyncInventory
                ]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, created) = app
        .post(
            "/api/secrets",
            Some(&member_token),
            json!({
                "name": "Member desktop",
                "scopes": [
                    SecretScope::SubscribeResources,
                    SecretScope::ReportTelemetry,
                    SecretScope::SyncInventory
                ]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert!(created["token"]
        .as_str()
        .is_some_and(|token| token.starts_with("evc_")));
    let secret_id = created["secret"]["id"].as_str().expect("secret id");

    let (status, listed) = app
        .get(
            &format!("/api/members/{}/secrets", member.id),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
    assert!(listed[0].get("token").is_none());
    assert!(listed[0].get("token_hash").is_none());

    let (status, revoked) = app
        .post(
            &format!("/api/members/{}/secrets/{}/revoke", member.id, secret_id),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{revoked}");
    assert_eq!(revoked["revoked"], true);
}

#[tokio::test]
async fn token_issuance_rejects_duplicate_scopes_without_persisting_a_secret() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let member_token = app.token_for(&member).await;

    let (status, body) = app
        .post(
            "/api/secrets",
            Some(&member_token),
            json!({
                "name": "Duplicate scope token",
                "scopes": [
                    SecretScope::SubscribeResources,
                    SecretScope::SubscribeResources
                ]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(app
        .state
        .db
        .secrets()
        .list_for_user(member.id)
        .await
        .expect("list member secrets")
        .is_empty());
}

#[tokio::test]
async fn contributors_and_other_members_cannot_manage_member_tokens() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let other = app.seed_user(PrimaryRole::User).await;
    let contributor = app.seed_user(PrimaryRole::Contribute).await;

    for actor in [&other, &contributor] {
        let token = app.token_for(actor).await;
        let (status, _) = app
            .get(&format!("/api/members/{}/secrets", member.id), Some(&token))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn self_revoke_audits_the_credential_id_without_exposing_token_material() {
    let observer = Arc::new(RecordingObserver::default());
    let app = test_app_with_authorization(AuthorizationService::new(observer.clone())).await;
    app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let member_token = app.token_for(&member).await;
    let (status, created) = app
        .post(
            "/api/secrets",
            Some(&member_token),
            json!({"name": "Audited token", "scopes": [SecretScope::SubscribeResources]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let secret_id = Uuid::parse_str(created["secret"]["id"].as_str().expect("secret id"))
        .expect("UUID secret id");

    let (status, body) = app
        .post(
            &format!("/api/secrets/{secret_id}/revoke"),
            Some(&member_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let events = observer.0.lock().expect("observer lock");
    let event = events
        .iter()
        .find(|event| {
            event.stage == AuthorizationStage::Target
                && event.action == AuthorizationAction::ConnectionTokensSelfRevoke
        })
        .expect("self revoke target decision");
    assert_eq!(event.target_id, Some(secret_id));
    assert_eq!(event.actor_id, member.id);
}
