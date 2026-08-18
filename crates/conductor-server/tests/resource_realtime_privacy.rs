mod support;

use std::time::Duration;

use axum::http::StatusCode;
use conductor_domain::{
    CreateResourceRequest, PrimaryRole, ResourceAccessPolicy, ResourceKind, ResourceVisibility,
    SetupRequest,
};
use conductor_server::http::realtime::{RealtimeAudience, RealtimeSignal};
use conductor_storage::repos::DraftContent;
use serde_json::json;
use support::{test_app, TestApp};
use tokio::sync::broadcast::{error::TryRecvError, Receiver};
use uuid::Uuid;

#[tokio::test]
async fn private_catalog_mutations_remove_only_the_previous_audience() {
    let app = test_app().await;
    let (project, _) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "private-realtime-catalog".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "realtime-admin@example.test".into(),
                admin_display_name: "Realtime Admin".into(),
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
    let previous_member = app.seed_user(PrimaryRole::User).await;
    let current_member = app.seed_user(PrimaryRole::User).await;
    let owner_token = app.token_for(&owner).await;
    let resource_id = seed_published_private_resource(&app, project.id, owner.id).await;
    let resource_path = format!("/api/resources/{resource_id}");
    let access_path = format!("{resource_path}/access");
    let mut receiver = app.state.realtime.subscribe();

    let (status, body) = app
        .patch(
            &resource_path,
            Some(&owner_token),
            json!({
                "name": "Private catalog resource updated",
                "description": null,
                "visibility": null
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_owner_removal(next_signal(&mut receiver).await, resource_id, owner.id);
    assert_owner_upsert(next_signal(&mut receiver).await, resource_id, owner.id);
    assert_empty(&mut receiver);

    let (status, body) = app
        .put(
            &access_path,
            Some(&owner_token),
            access_policy(&[previous_member.id]),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_owner_removal(next_signal(&mut receiver).await, resource_id, owner.id);
    assert_policy_upsert(
        next_signal(&mut receiver).await,
        resource_id,
        owner.id,
        &[previous_member.id],
    );
    assert_empty(&mut receiver);

    let (status, body) = app
        .put(
            &access_path,
            Some(&owner_token),
            access_policy(&[current_member.id]),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_policy_removal(
        next_signal(&mut receiver).await,
        resource_id,
        owner.id,
        &[previous_member.id],
    );
    assert_policy_upsert(
        next_signal(&mut receiver).await,
        resource_id,
        owner.id,
        &[current_member.id],
    );
    assert_empty(&mut receiver);

    let (status, body) = app
        .post(
            &format!("{resource_path}/archive"),
            Some(&owner_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_policy_removal(
        next_signal(&mut receiver).await,
        resource_id,
        owner.id,
        &[current_member.id],
    );
    assert_empty(&mut receiver);
}

async fn seed_published_private_resource(app: &TestApp, project_id: Uuid, owner_id: Uuid) -> Uuid {
    let resource = app
        .state
        .db
        .resources()
        .create(
            project_id,
            &CreateResourceRequest {
                kind: ResourceKind::Agent,
                slug: "private-realtime-agent".into(),
                name: "Private realtime agent".into(),
                description: None,
                version: "0.1.0".into(),
                visibility: ResourceVisibility::Private,
                payload: json!({}),
                changelog: None,
            },
            owner_id,
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
        .expect("create private resource");
    sqlx::query("UPDATE resources SET status = 'published' WHERE id = ?")
        .bind(resource.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("publish private resource");
    resource.id
}

fn access_policy(member_ids: &[Uuid]) -> serde_json::Value {
    json!({
        "all_members": false,
        "primary_roles": [],
        "sub_role_ids": [],
        "tag_ids": [],
        "member_ids": member_ids
    })
}

async fn next_signal(
    receiver: &mut Receiver<conductor_server::http::realtime::RealtimeMessage>,
) -> RealtimeSignal {
    tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("realtime signal timeout")
        .expect("realtime channel closed")
        .signal
}

fn assert_empty(receiver: &mut Receiver<conductor_server::http::realtime::RealtimeMessage>) {
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

fn assert_owner_removal(signal: RealtimeSignal, resource_id: Uuid, owner_id: Uuid) {
    match signal {
        RealtimeSignal::ResourceDelete {
            audience: RealtimeAudience::Owner(actual_owner_id),
            resource_id: actual_resource_id,
        } => {
            assert_eq!(actual_resource_id, resource_id);
            assert_eq!(actual_owner_id, owner_id);
        }
        other => panic!("expected owner-only resource removal, got {other:?}"),
    }
}

fn assert_owner_upsert(signal: RealtimeSignal, resource_id: Uuid, owner_id: Uuid) {
    match signal {
        RealtimeSignal::ResourceUpsert {
            audience: RealtimeAudience::Owner(actual_owner_id),
            resource,
        } => {
            assert_eq!(resource.id, resource_id);
            assert_eq!(actual_owner_id, owner_id);
        }
        other => panic!("expected owner-only resource upsert, got {other:?}"),
    }
}

fn assert_policy_removal(
    signal: RealtimeSignal,
    resource_id: Uuid,
    owner_id: Uuid,
    member_ids: &[Uuid],
) {
    match signal {
        RealtimeSignal::ResourceDelete {
            audience:
                RealtimeAudience::Policy {
                    owner_user_id,
                    policy,
                },
            resource_id: actual_resource_id,
        } => {
            assert_eq!(actual_resource_id, resource_id);
            assert_policy(owner_user_id, &policy, owner_id, member_ids);
        }
        other => panic!("expected policy-scoped resource removal, got {other:?}"),
    }
}

fn assert_policy_upsert(
    signal: RealtimeSignal,
    resource_id: Uuid,
    owner_id: Uuid,
    member_ids: &[Uuid],
) {
    match signal {
        RealtimeSignal::ResourceUpsert {
            audience:
                RealtimeAudience::Policy {
                    owner_user_id,
                    policy,
                },
            resource,
        } => {
            assert_eq!(resource.id, resource_id);
            assert_policy(owner_user_id, &policy, owner_id, member_ids);
        }
        other => panic!("expected policy-scoped resource upsert, got {other:?}"),
    }
}

fn assert_policy(
    actual_owner_id: Uuid,
    policy: &ResourceAccessPolicy,
    owner_id: Uuid,
    member_ids: &[Uuid],
) {
    assert_eq!(actual_owner_id, owner_id);
    assert!(!policy.all_members);
    assert!(policy.primary_roles.is_empty());
    assert!(policy.sub_role_ids.is_empty());
    assert!(policy.tag_ids.is_empty());
    assert_eq!(policy.member_ids, member_ids);
}
