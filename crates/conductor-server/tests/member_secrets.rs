mod support;

use axum::http::StatusCode;
use conductor_domain::{PrimaryRole, SecretScope};
use serde_json::json;
use support::test_app;

#[tokio::test]
async fn admin_can_manage_member_tokens_without_revealing_them_again() {
    let app = test_app().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;

    let (status, created) = app
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
async fn contributors_and_other_members_cannot_manage_member_tokens() {
    let app = test_app().await;
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
