mod support;

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use conductor_domain::{
    AuthorizationAction, DecisionReason, PermissionKey, PrimaryRole, SetupRequest,
};
use conductor_server::core::authorization::{
    AuthorizationDecisionObserver, AuthorizationEvent, AuthorizationResult, AuthorizationService,
    AuthorizationStage, MemberAccessChangeEvent,
};
use serde_json::json;
use support::{test_app, test_app_with_authorization};
use uuid::Uuid;

#[derive(Default)]
struct RecordingObserver {
    decisions: Mutex<Vec<AuthorizationEvent>>,
    member_changes: Mutex<Vec<MemberAccessChangeEvent>>,
}

impl AuthorizationDecisionObserver for RecordingObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        self.decisions
            .lock()
            .expect("observer lock")
            .push(event.clone());
    }

    fn observe_member_access_change(&self, event: &MemberAccessChangeEvent) {
        self.member_changes
            .lock()
            .expect("member observer lock")
            .push(event.clone());
    }
}

#[tokio::test]
async fn corrupt_project_identity_fails_closed_instead_of_becoming_an_unconfigured_project() {
    let app = test_app().await;
    app.state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "strict-project-target".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "strict-project-admin@example.test".into(),
                admin_display_name: "Strict Project Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let member = app.seed_user(PrimaryRole::User).await;
    let token = app.token_for(&member).await;
    sqlx::query("UPDATE instance SET id = 'corrupt-project-identity'")
        .execute(app.state.db.pool())
        .await
        .expect("corrupt project UUID");

    let (status, body) = app
        .get(&format!("/api/members/{}", member.id), Some(&token))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error_code"], "unauthorized");
    assert!(!body.to_string().contains("corrupt-project-identity"));
}

#[tokio::test]
async fn member_private_reads_resolve_self_other_and_admin_at_target_stage() {
    let observer = Arc::new(RecordingObserver::default());
    let app = test_app_with_authorization(AuthorizationService::new(observer.clone())).await;
    app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let contributor = app.seed_user(PrimaryRole::Contribute).await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let member_token = app.token_for(&member).await;
    let contributor_token = app.token_for(&contributor).await;
    let admin_token = app.token_for(&admin).await;
    let path = format!("/api/members/{}", member.id);

    assert_eq!(app.get(&path, Some(&member_token)).await.0, StatusCode::OK);
    assert_eq!(
        app.get(&path, Some(&contributor_token)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(app.get(&path, Some(&admin_token)).await.0, StatusCode::OK);

    let events = observer.decisions.lock().expect("observer lock");
    let target_events = events
        .iter()
        .filter(|event| {
            event.stage == AuthorizationStage::Target
                && event.action == AuthorizationAction::MemberPrivateRead
        })
        .collect::<Vec<_>>();
    assert_eq!(target_events.len(), 3, "{target_events:#?}");
    assert!(target_events.iter().any(|event| {
        event.actor_id == member.id
            && event.authorization_result == AuthorizationResult::Allowed
            && event.resolved_permission == Some(PermissionKey::MemberPrivateReadSelf)
    }));
    assert!(target_events.iter().any(|event| {
        event.actor_id == contributor.id
            && event.authorization_result == AuthorizationResult::Denied
            && event.reason_code == Some(DecisionReason::DenyNotSelf)
    }));
    assert!(target_events.iter().any(|event| {
        event.actor_id == admin.id
            && event.authorization_result == AuthorizationResult::Allowed
            && event.resolved_permission == Some(PermissionKey::MemberPrivateReadAny)
    }));
}

#[tokio::test]
async fn committed_member_change_is_observed_with_sealed_route_context_and_stable_errors() {
    let observer = Arc::new(RecordingObserver::default());
    let app = test_app_with_authorization(AuthorizationService::new(observer.clone())).await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "member-change-observer".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "member-change-admin@example.test".into(),
                admin_display_name: "Member Change Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let target = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;

    let (status, updated) = app
        .patch(
            &format!("/api/members/{}", target.id),
            Some(&admin_token),
            json!({"primary_role": "contribute"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");

    {
        let changes = observer
            .member_changes
            .lock()
            .expect("member observer lock");
        assert_eq!(changes.len(), 1, "{changes:#?}");
        let change = &changes[0];
        assert_eq!(
            change.action,
            AuthorizationAction::MemberAccessProfileUpdate
        );
        assert_eq!(change.actor_id, admin.id);
        assert_eq!(change.target_id, target.id);
        assert_eq!(change.before.primary_role, PrimaryRole::User);
        assert_eq!(change.after.primary_role, PrimaryRole::Contribute);
        assert_eq!(change.normalized_route_id, "member.access_profile.update");
    }

    let (status, body) = app
        .patch(
            &format!("/api/members/{}", admin.id),
            Some(&admin_token),
            json!({"primary_role": "user"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error_code"], "self_role_change");

    let (status, body) = app
        .post(
            &format!("/api/members/{}/disable", admin.id),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error_code"], "self_disable");

    let (status, body) = app
        .patch(
            &format!("/api/members/{}", Uuid::new_v4()),
            Some(&admin_token),
            json!({"display_name": "Missing"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error_code"], "member_not_found");
}
