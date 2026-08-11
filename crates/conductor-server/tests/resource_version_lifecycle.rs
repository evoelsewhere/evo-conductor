mod support;

use axum::http::StatusCode;
use conductor_domain::{
    CreateResourceRequest, PrimaryRole, ReleaseChannel, ReleaseResourceRequest, ResourceKind,
    ResourceVisibility, SetupRequest, VersionMode,
};
use conductor_storage::repos::ReleaseContent;
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
        let first_payload = serde_json::json!({
            "files": [{"path": source_path, "content": first_content}]
        });
        let second_payload = serde_json::json!({
            "files": [{"path": source_path, "content": second_content}]
        });
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
                    payload: first_payload.clone(),
                    changelog: None,
                },
                admin.id,
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
                &release_content('a', &first_payload),
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
                &release_content('b', &second_payload),
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

fn release_content(marker: char, payload: &serde_json::Value) -> ReleaseContent {
    ReleaseContent {
        sha256: marker.to_string().repeat(64),
        size: payload.to_string().len().try_into().unwrap_or(u64::MAX),
        artifact_key: None,
        updated_payload: Some(payload.to_string()),
    }
}
