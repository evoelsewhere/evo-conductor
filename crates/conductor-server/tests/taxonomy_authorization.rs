mod support;

use axum::http::StatusCode;
use conductor_domain::{
    CreateResourceRequest, PrimaryRole, ResourceKind, ResourceVisibility, SetupRequest,
};
use conductor_storage::repos::DraftContent;
use serde_json::json;
use support::test_app;

#[tokio::test]
async fn contributor_reads_taxonomy_but_only_admin_mutates_definitions_and_member_tags() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let contributor = app.seed_user(PrimaryRole::Contribute).await;
    let member = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;
    let contributor_token = app.token_for(&contributor).await;
    let member_token = app.token_for(&member).await;

    let (status, created) = app
        .post(
            "/api/tags",
            Some(&admin_token),
            json!({
                "slug": "platform",
                "name": "Platform",
                "description": null,
                "color": "#4c66d6"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let tag_id = created["id"].as_str().expect("tag id");

    for token in [&admin_token, &contributor_token] {
        let (status, tags) = app.get("/api/tags", Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{tags}");
        assert_eq!(tags.as_array().map(Vec::len), Some(1));

        let (status, sub_roles) = app.get("/api/sub-roles", Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{sub_roles}");
    }
    assert_eq!(
        app.get("/api/tags", Some(&member_token)).await.0,
        StatusCode::FORBIDDEN
    );

    let (status, _) = app
        .post(
            "/api/tags",
            Some(&contributor_token),
            json!({
                "slug": "security",
                "name": "Security",
                "description": null,
                "color": null
            }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let member_tags_path = format!("/api/tag-assignments/member/{}", member.id);
    let assignment = json!({ "tag_ids": [tag_id] });
    let (status, _) = app
        .put(
            &member_tags_path,
            Some(&contributor_token),
            assignment.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, assigned) = app
        .put(&member_tags_path, Some(&admin_token), assignment)
        .await;
    assert_eq!(status, StatusCode::OK, "{assigned}");
    assert_eq!(assigned["tag_ids"][0], tag_id);

    let (status, body) = app
        .delete(
            &format!("/api/tags/{tag_id}"),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, _) = app
        .put(
            &member_tags_path,
            Some(&admin_token),
            json!({ "tag_ids": [] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = app
        .delete(
            &format!("/api/tags/{tag_id}"),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn tag_assignment_rejects_unclassified_entity_types() {
    let app = test_app().await;
    let admin_token = app.token_for_role(PrimaryRole::Admin).await;
    let (status, body) = app
        .put(
            "/api/tag-assignments/arbitrary/not-a-target",
            Some(&admin_token),
            json!({ "tag_ids": [] }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn resource_tag_assignment_requires_admin_or_the_owning_contributor() {
    let app = test_app().await;
    let (project, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "resource-tag-authorization".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "resource-tag-admin@example.test".into(),
                admin_display_name: "Resource Tag Admin".into(),
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
    let user = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;
    let owner_token = app.token_for(&owner).await;
    let peer_token = app.token_for(&peer).await;
    let user_token = app.token_for(&user).await;

    let (status, created_tag) = app
        .post(
            "/api/tags",
            Some(&admin_token),
            json!({
                "slug": "owned-resource",
                "name": "Owned resource",
                "description": null,
                "color": null
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created_tag}");
    let tag_id = created_tag["id"].as_str().expect("tag id");
    let resource = app
        .state
        .db
        .resources()
        .create(
            project.id,
            &CreateResourceRequest {
                kind: ResourceKind::Agent,
                slug: "owned-agent".into(),
                name: "Owned agent".into(),
                description: None,
                version: "0.1.0".into(),
                visibility: ResourceVisibility::Shared,
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
        .expect("create owned resource");
    let path = format!("/api/tag-assignments/resource/{}", resource.id);

    let (status, assigned) = app
        .put(&path, Some(&owner_token), json!({ "tag_ids": [tag_id] }))
        .await;
    assert_eq!(status, StatusCode::OK, "{assigned}");
    assert_eq!(assigned["tag_ids"][0], tag_id);

    assert_eq!(
        app.get(&path, Some(&peer_token)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.put(&path, Some(&user_token), json!({ "tag_ids": [] }))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(app.get(&path, Some(&admin_token)).await.0, StatusCode::OK);
}
