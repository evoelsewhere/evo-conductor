mod support;

use std::io::{Cursor, Write};

use axum::http::StatusCode;
use conductor_domain::{PrimaryRole, SetupRequest};
use support::test_app;

fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (path, content) in entries {
        writer.start_file(path, options).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

async fn configured_app() -> (support::TestApp, String) {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "resource-import-test".into(),
                display_name: Some("Resource import test".into()),
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
    (app, token)
}

#[tokio::test]
async fn imports_a_wrapped_evoflux_agent_markdown_package() {
    let (app, token) = configured_app().await;
    let package = archive(&[(
        "release-review/release_review.md",
        "---\nname: release_review\nrole: member\ndescription: Reviews release readiness.\n---\n\nYou review release readiness.\n",
    )]);

    let (status, inspection) = app
        .post_bytes(
            "/api/resources/imports/agent/inspect",
            Some(&token),
            "application/zip",
            package.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{inspection}");
    assert_eq!(inspection["metadata"]["slug"], "release_review");
    assert_eq!(
        inspection["metadata"]["primary_source"],
        "release_review.md"
    );
    assert_eq!(inspection["validation"]["valid"], true);

    let (status, created) = app
        .post_bytes(
            "/api/resources/imports/agent?slug=release_review&name=Release%20Review&visibility=shared",
            Some(&token),
            "application/zip",
            package,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["resource"]["kind"], "agent");
    assert_eq!(created["resource"]["slug"], "release_review");
    assert_eq!(created["validation"]["valid"], true);

    let resource_id = created["resource"]["id"].as_str().unwrap();
    let (status, tree) = app
        .get(
            &format!("/api/resources/{resource_id}/draft/files"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{tree}");
    assert!(tree["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == "release_review.md"));
    assert!(tree["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == ".evoflux.json"));
}

#[tokio::test]
async fn imports_a_complete_evoflux_skill_bundle() {
    let (app, token) = configured_app().await;
    let package = archive(&[
        (
            "incident-summary/SKILL.md",
            "---\nname: incident-summary\ndescription: Summarize incidents with evidence and clear follow-ups.\n---\n\n# Incident summary\n\nUse this workflow when an incident needs a concise evidence-backed summary.\n",
        ),
        (
            "incident-summary/agents/evoflux.yaml",
            "interface:\n  display_name: Incident summary\n  short_description: Summarize incidents\n",
        ),
        (
            "incident-summary/evals/trigger-cases.json",
            "{\"skill\":\"incident-summary\",\"cases\":[]}",
        ),
        (
            "incident-summary/references/template.md",
            "# Incident template\n",
        ),
    ]);

    let (status, inspection) = app
        .post_bytes(
            "/api/resources/imports/skill/inspect",
            Some(&token),
            "application/zip",
            package,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{inspection}");
    assert_eq!(inspection["metadata"]["slug"], "incident-summary");
    assert_eq!(inspection["file_count"], 4);
    assert_eq!(inspection["validation"]["valid"], true);
}

#[tokio::test]
async fn safe_but_invalid_skill_zip_becomes_a_repairable_draft() {
    let (app, token) = configured_app().await;
    let package = archive(&[(
        "SKILL.md",
        "---\nname: wrong-name\ncustom: unsupported\n---\n",
    )]);

    let (status, inspection) = app
        .post_bytes(
            "/api/resources/imports/skill/inspect",
            Some(&token),
            "application/zip",
            package.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{inspection}");
    assert_eq!(inspection["validation"]["valid"], false);

    let (status, created) = app
        .post_bytes(
            "/api/resources/imports/skill?slug=repair-me&name=Repair%20Me&visibility=private",
            Some(&token),
            "application/zip",
            package,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["resource"]["status"], "draft");
    assert_eq!(created["validation"]["valid"], false);
}

#[tokio::test]
async fn regular_members_cannot_inspect_agent_or_skill_archives() {
    let (app, _) = configured_app().await;
    let token = app.token_for_role(PrimaryRole::User).await;
    let package = archive(&[(
        "agent.md",
        "---\nname: agent\nrole: member\n---\n\nPrompt\n",
    )]);
    let (status, _) = app
        .post_bytes(
            "/api/resources/imports/agent/inspect",
            Some(&token),
            "application/zip",
            package,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
