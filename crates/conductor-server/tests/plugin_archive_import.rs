mod support;

use std::io::{Cursor, Write};

use axum::http::StatusCode;
use conductor_domain::{PrimaryRole, SetupRequest};
use support::test_app;

const PLUGIN_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";
const PLUGIN_NAME: &str = "release-readiness";

fn plugin_archive(manifest: serde_json::Value) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("plugin.json", options).unwrap();
    writer
        .write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes())
        .unwrap();
    writer
        .start_file("skills/release-readiness/SKILL.md", options)
        .unwrap();
    writer
        .write_all(
            b"---\nname: release-readiness\ndescription: Review a release.\n---\n\n# Release readiness\n",
        )
        .unwrap();
    writer.finish().unwrap().into_inner()
}

async fn configured_app() -> (support::TestApp, conductor_domain::User, String) {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "plugin-import-test".into(),
                display_name: Some("Plugin import test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "admin@example.test".into(),
                admin_display_name: "Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let token = app.token_for(&admin).await;
    (app, admin, token)
}

#[tokio::test]
async fn inspects_then_atomically_creates_a_plugin_draft() {
    let (app, _, token) = configured_app().await;
    let archive = plugin_archive(serde_json::json!({
        "$schema": PLUGIN_SCHEMA,
        "name": PLUGIN_NAME,
        "version": "0.4.2",
        "description": "Review releases before shipping.",
        "extensions": {}
    }));

    let (status, inspection) = app
        .post_bytes(
            "/api/resources/plugins/inspect",
            Some(&token),
            "application/zip",
            archive.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{inspection}");
    assert_eq!(inspection["manifest"]["name"], PLUGIN_NAME);
    assert_eq!(inspection["manifest"]["version"], "0.4.2");
    assert_eq!(inspection["file_count"], 2);
    assert_eq!(inspection["skill_count"], 1);
    assert_eq!(inspection["validation"]["valid"], true);

    let (status, created) = app
        .post_bytes(
            "/api/resources/plugins/import?name=Release%20Readiness&visibility=shared",
            Some(&token),
            "application/zip",
            archive,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["resource"]["kind"], "plugin");
    assert_eq!(created["resource"]["slug"], PLUGIN_NAME);
    assert_eq!(created["resource"]["name"], "Release Readiness");
    assert_eq!(created["resource"]["version"], "0.4.2");
    assert_eq!(created["resource"]["status"], "draft");
    assert_eq!(created["validation"]["valid"], true);

    let resource_id = created["resource"]["id"].as_str().unwrap();
    let (status, tree) = app
        .get(
            &format!("/api/resources/{resource_id}/draft/files"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{tree}");
    assert_eq!(tree["files"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn manages_draft_files_with_revision_conflicts_and_root_protection() {
    let (app, _, token) = configured_app().await;
    let archive = plugin_archive(serde_json::json!({
        "$schema": PLUGIN_SCHEMA,
        "name": PLUGIN_NAME,
        "version": "0.4.2",
        "description": "Review releases before shipping.",
        "extensions": {}
    }));
    let (status, created) = app
        .post_bytes(
            "/api/resources/plugins/import?name=Release%20Readiness&visibility=shared",
            Some(&token),
            "application/zip",
            archive,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let resource_id = created["resource"]["id"].as_str().unwrap();
    let entries_path = format!("/api/resources/{resource_id}/draft/entries");

    let (status, created_file) = app
        .post(
            &entries_path,
            Some(&token),
            serde_json::json!({
                "path": "commands/check.md",
                "content": "# Check\n",
                "draft_revision": 0
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created_file}");
    assert_eq!(created_file["revision"], 1);

    let (status, moved) = app
        .patch(
            &entries_path,
            Some(&token),
            serde_json::json!({
                "path": "commands",
                "destination_path": "command-templates",
                "draft_revision": 1
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert!(moved["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == "command-templates/check.md"));

    let (status, deleted) = app
        .delete(
            &entries_path,
            Some(&token),
            serde_json::json!({
                "path": "command-templates",
                "draft_revision": 2
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert_eq!(deleted["revision"], 3);

    let (status, protected) = app
        .delete(
            &entries_path,
            Some(&token),
            serde_json::json!({
                "path": "plugin.json",
                "draft_revision": 3
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{protected}");

    let (status, conflict) = app
        .post(
            &entries_path,
            Some(&token),
            serde_json::json!({
                "path": "README.md",
                "content": "# Read me\n",
                "draft_revision": 0
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
}

#[tokio::test]
async fn rejects_invalid_archives_without_leaving_a_resource() {
    let (app, _, token) = configured_app().await;
    let archive = plugin_archive(serde_json::json!({
        "$schema": PLUGIN_SCHEMA,
        "name": PLUGIN_NAME,
        "version": "not-semver",
        "description": "Invalid version",
        "extensions": {}
    }));

    let (status, inspection) = app
        .post_bytes(
            "/api/resources/plugins/inspect",
            Some(&token),
            "application/zip",
            archive.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{inspection}");
    assert_eq!(inspection["validation"]["valid"], false);

    let (status, error) = app
        .post_bytes(
            "/api/resources/plugins/import?name=Invalid&visibility=shared",
            Some(&token),
            "application/zip",
            archive,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(error["error"]
        .as_str()
        .unwrap()
        .contains("manifest_version_invalid"));
    assert!(app
        .state
        .db
        .resources()
        .list_all()
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn regular_members_cannot_inspect_plugin_packages() {
    let (app, _, _) = configured_app().await;
    let user_token = app.token_for_role(PrimaryRole::User).await;
    let archive = plugin_archive(serde_json::json!({
        "$schema": PLUGIN_SCHEMA,
        "name": PLUGIN_NAME,
        "version": "0.1.0",
        "description": "Not authorized",
        "extensions": {}
    }));

    let (status, _) = app
        .post_bytes(
            "/api/resources/plugins/inspect",
            Some(&user_token),
            "application/zip",
            archive,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
