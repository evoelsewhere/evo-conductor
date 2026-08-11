mod support;

use axum::http::StatusCode;
use conductor_domain::{
    CreateResourceRequest, DraftFile, PrimaryRole, ReleaseChannel, ReleaseResourceRequest,
    ResourceKind, ResourceVisibility, SetupRequest, VersionMode,
};
use conductor_server::core::resource_authoring::{
    resource_archive_media_type, resource_storage_payload,
};
use conductor_storage::repos::{DraftContent, ReleaseContent};
use support::test_app;

#[tokio::test]
async fn version_lifecycle_endpoints_cover_agent_and_skill_source_shapes() {
    let app = test_app().await;
    let (project, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "version-lifecycle-api".into(),
                display_name: Some("Version lifecycle API".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "version-admin@example.test".into(),
                admin_display_name: "Version Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let user = app.seed_user(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;
    let user_token = app.token_for(&user).await;
    let cases = [
        (
            ResourceKind::Agent,
            "version-api-agent",
            "Version API Agent",
            "version-api-agent.md",
        ),
        (
            ResourceKind::Skill,
            "version-api-skill",
            "Version API Skill",
            "SKILL.md",
        ),
    ];

    for (kind, slug, name, source_path) in cases {
        let first_content = format!("first {} API release", kind.as_str());
        let second_content = format!("second {} API release", kind.as_str());
        let first_stored =
            stored_content(&app, kind, slug, "0.1.0", source_path, &first_content).await;
        let second_stored =
            stored_content(&app, kind, slug, "0.1.1", source_path, &second_content).await;
        let resource = app
            .state
            .db
            .resources()
            .create(
                project.id,
                &CreateResourceRequest {
                    kind,
                    slug: slug.into(),
                    name: name.into(),
                    description: None,
                    version: "0.1.0".into(),
                    visibility: ResourceVisibility::Shared,
                    payload: first_stored.metadata_payload.clone(),
                    changelog: None,
                },
                admin.id,
                &first_stored,
            )
            .await
            .expect("create resource");
        let first = app
            .state
            .db
            .resources()
            .release(
                resource.id,
                &release_request(0),
                &release_content(&first_stored),
                admin.id,
            )
            .await
            .expect("release first");
        let second = app
            .state
            .db
            .resources()
            .release(
                resource.id,
                &release_request(1),
                &release_content(&second_stored),
                admin.id,
            )
            .await
            .expect("release second");

        let active_path = format!(
            "/api/resources/{}/versions/{}/deprecate",
            resource.id, second.version_id
        );
        let (status, body) = app
            .post(
                &active_path,
                Some(&admin_token),
                serde_json::json!({"reason": "active version"}),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(
            body["error"],
            "conflict: active_release_cannot_be_deprecated"
        );

        let deprecate_path = format!(
            "/api/resources/{}/versions/{}/deprecate",
            resource.id, first.version_id
        );
        let (status, _) = app
            .post(
                &deprecate_path,
                None,
                serde_json::json!({"reason": "compatibility issue"}),
            )
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = app
            .post(
                &deprecate_path,
                Some(&user_token),
                serde_json::json!({"reason": "compatibility issue"}),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, deprecated) = app
            .post(
                &deprecate_path,
                Some(&admin_token),
                serde_json::json!({"reason": "compatibility issue"}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{deprecated}");
        assert_eq!(deprecated["status"], "deprecated");
        assert_eq!(deprecated["deprecation_reason"], "compatibility issue");
        assert_eq!(deprecated["active_channel"], serde_json::Value::Null);

        let restore_path = format!(
            "/api/resources/{}/versions/{}/restore-to-draft",
            resource.id, first.version_id
        );
        let (status, body) = app
            .post(
                &restore_path,
                Some(&admin_token),
                serde_json::json!({"draft_revision": 2, "confirm_deprecated": false}),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(
            body["error"],
            "conflict: deprecated_version_confirmation_required"
        );

        let (status, restored) = app
            .post(
                &restore_path,
                Some(&admin_token),
                serde_json::json!({"draft_revision": 2, "confirm_deprecated": true}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{restored}");
        assert_eq!(restored["revision"], 3);
        assert_eq!(restored["files"][0]["path"], source_path);
        assert_eq!(restored["files"][0]["content"], first_content);
    }
}

fn release_request(draft_revision: u64) -> ReleaseResourceRequest {
    ReleaseResourceRequest {
        channel: ReleaseChannel::Published,
        version_mode: VersionMode::Auto,
        manual_version: None,
        draft_revision,
        changelog: None,
        beta_member_ids: vec![],
        minimum_evoflux_version: None,
    }
}

fn release_content(stored: &DraftContent) -> ReleaseContent {
    ReleaseContent {
        sha256: stored.sha256.clone(),
        size: stored.size,
        artifact_key: Some(stored.artifact_key.clone()),
        updated_payload: Some(stored.metadata_payload.to_string()),
    }
}

async fn stored_content(
    app: &support::TestApp,
    kind: ResourceKind,
    slug: &str,
    version: &str,
    path: &str,
    content: &str,
) -> DraftContent {
    let files = vec![DraftFile {
        path: path.into(),
        content: content.into(),
    }];
    let artifact = app.state.artifacts.put_bundle(&files).await.unwrap();
    DraftContent {
        metadata_payload: resource_storage_payload(
            kind,
            slug,
            version,
            &artifact.key,
            &artifact.sha256,
            artifact.size,
            resource_archive_media_type(kind),
            &files,
        ),
        artifact_key: artifact.key,
        sha256: artifact.sha256,
        size: artifact.size,
    }
}
