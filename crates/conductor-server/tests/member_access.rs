mod support;

use std::time::Duration;

use axum::http::StatusCode;
use chrono::Utc;
use conductor_domain::{PrimaryRole, SecretScope};
use conductor_server::http::realtime::RealtimeSignal;
use serde_json::json;
use support::{test_app, TestApp};
use uuid::Uuid;

async fn seed_instance(app: &TestApp) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, bind_host, bind_port, collection_level,
            setup_completed, jwt_secret, created_at, updated_at
        ) VALUES (?, 'Member access API test', '127.0.0.1', 0, 'L1', 1, 'unused', ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed singleton instance");
}

#[tokio::test]
async fn unchanged_browser_credential_tracks_routine_roles_but_admin_elevation_requires_login() {
    let app = test_app().await;
    seed_instance(&app).await;
    let actor = app.seed_user(PrimaryRole::Admin).await;
    let target = app.seed_user(PrimaryRole::User).await;
    let actor_token = app.token_for(&actor).await;
    let unchanged_target_token = app.token_for(&target).await;

    let (status, _) = app
        .get("/api/dashboard", Some(&unchanged_target_token))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let mut realtime = app.state.realtime.subscribe();
    let (status, updated) = app
        .patch(
            &format!("/api/members/{}", target.id),
            Some(&actor_token),
            json!({"primary_role": "contribute"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["primary_role"], "contribute");
    let signal = realtime.recv().await.expect("post-commit audience signal");
    assert!(matches!(
        signal.signal,
        RealtimeSignal::ResourceAudienceChanged { owner_user_id }
            if owner_user_id == target.id
    ));
    let (status, _) = app
        .get("/api/dashboard", Some(&unchanged_target_token))
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, updated) = app
        .patch(
            &format!("/api/members/{}", target.id),
            Some(&actor_token),
            json!({"primary_role": "user"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (status, _) = app
        .get("/api/dashboard", Some(&unchanged_target_token))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, updated) = app
        .patch(
            &format!("/api/members/{}", target.id),
            Some(&actor_token),
            json!({"primary_role": "admin"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (status, _) = app.get("/api/auth/me", Some(&unchanged_target_token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let current_target = app
        .state
        .db
        .users()
        .find_by_id(target.id)
        .await
        .expect("read elevated member")
        .expect("target exists");
    let fresh_target_token = app.token_for(&current_target).await;
    let (status, _) = app.get("/api/settings", Some(&fresh_target_token)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn disable_invalidates_browser_session_after_commit_without_claiming_req005_revocation() {
    let app = test_app().await;
    seed_instance(&app).await;
    let actor = app.seed_user(PrimaryRole::Admin).await;
    let target = app.seed_user(PrimaryRole::User).await;
    let actor_token = app.token_for(&actor).await;
    let target_token = app.token_for(&target).await;
    let secret = app
        .state
        .db
        .secrets()
        .insert(
            target.id,
            "desktop",
            "ef_status",
            "member-access-server-status-token",
            &[SecretScope::SubscribeResources],
            None,
        )
        .await
        .expect("insert connection credential");

    let mut realtime = app.state.realtime.subscribe();
    let (status, disabled) = app
        .post(
            &format!("/api/members/{}/disable", target.id),
            Some(&actor_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{disabled}");
    assert_eq!(disabled["status"], "disabled");
    let signal = realtime
        .recv()
        .await
        .expect("post-commit disconnect signal");
    assert!(matches!(
        signal.signal,
        RealtimeSignal::AccessRevoked {
            owner_user_id: Some(owner_user_id),
            ..
        } if owner_user_id == target.id
    ));

    let (status, _) = app.get("/api/auth/me", Some(&target_token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(app
        .state
        .db
        .secrets()
        .is_active(secret.id)
        .await
        .expect("durable credential remains for REQ-005"));

    let (status, enabled) = app
        .post(
            &format!("/api/members/{}/enable", target.id),
            Some(&actor_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{enabled}");
    assert_eq!(enabled["status"], "active");
    let (status, _) = app.get("/api/auth/me", Some(&target_token)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "enable must not restore an invalidated browser session"
    );
}

#[tokio::test]
async fn rejected_profile_change_emits_no_realtime_side_effect() {
    let app = test_app().await;
    seed_instance(&app).await;
    let actor = app.seed_user(PrimaryRole::Admin).await;
    let target = app.seed_user(PrimaryRole::User).await;
    let actor_token = app.token_for(&actor).await;
    let mut realtime = app.state.realtime.subscribe();

    let (status, _) = app
        .patch(
            &format!("/api/members/{}", target.id),
            Some(&actor_token),
            json!({
                "primary_role": "contribute",
                "tag_ids": [Uuid::new_v4().to_string()]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), realtime.recv())
            .await
            .is_err(),
        "rollback must not publish an audience change"
    );
    let unchanged = app
        .state
        .db
        .users()
        .find_by_id(target.id)
        .await
        .expect("read target")
        .expect("target exists");
    assert_eq!(unchanged.primary_role, PrimaryRole::User);
}

#[tokio::test]
async fn contributor_member_list_is_active_directory_only_and_detail_is_self_or_admin() {
    let app = test_app().await;
    seed_instance(&app).await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let contributor = app.seed_user(PrimaryRole::Contribute).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let disabled_member = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;
    let contributor_token = app.token_for(&contributor).await;
    let (status, body) = app
        .post(
            &format!("/api/members/{}/disable", disabled_member.id),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, directory) = app.get("/api/members", Some(&contributor_token)).await;
    assert_eq!(status, StatusCode::OK, "{directory}");
    let entries = directory["items"].as_array().expect("directory items");
    let member_entry = entries
        .iter()
        .find(|entry| entry["id"] == member.id.to_string())
        .expect("member appears in active directory");
    let keys = member_entry
        .as_object()
        .expect("directory entry object")
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        ["display_name", "id", "primary_role"]
            .into_iter()
            .map(str::to_string)
            .collect()
    );
    assert!(entries
        .iter()
        .all(|entry| entry["id"] != disabled_member.id.to_string()));

    let (status, email_probe) = app
        .get(
            &format!("/api/members?q={}", member.email),
            Some(&contributor_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{email_probe}");
    assert!(email_probe["items"]
        .as_array()
        .expect("email-probe directory")
        .iter()
        .all(|entry| entry["id"] != member.id.to_string()));

    let (status, _) = app
        .get(
            &format!("/api/members/{}", member.id),
            Some(&contributor_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, own_detail) = app
        .get(
            &format!("/api/members/{}", contributor.id),
            Some(&contributor_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{own_detail}");
    assert_eq!(own_detail["email"], contributor.email);

    let (status, managed_detail) = app
        .get(&format!("/api/members/{}", member.id), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{managed_detail}");
    assert_eq!(managed_detail["email"], member.email);

    let plain_user = app.seed_user(PrimaryRole::User).await;
    let plain_user_token = app.token_for(&plain_user).await;
    let (status, _) = app.get("/api/members", Some(&plain_user_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn invalid_member_directory_policy_filters_fail_closed() {
    let app = test_app().await;
    seed_instance(&app).await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let token = app.token_for(&admin).await;

    for query in ["status=future_status", "role=super_admin"] {
        let (status, body) = app
            .get(&format!("/api/members?{query}"), Some(&token))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error_code"], "invalid_request");
    }
}
