mod support;

use axum::http::StatusCode;
use conductor_domain::{PrimaryRole, SetupRequest};
use conductor_server::{AppState, RealtimeConfig};
use conductor_storage::Db;
use sqlx::Row;
use std::process::Command;
use support::test_app;

#[tokio::test]
async fn admin_migrates_project_objects_between_local_backends() {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "object-storage-test".into(),
                display_name: Some("Object storage test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "storage-admin@example.test".into(),
                admin_display_name: "Storage Admin".into(),
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

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    png.extend_from_slice(b"object-storage-test-logo");
    let (status, settings) = app
        .put_bytes("/api/settings/logo", Some(&token), "image/png", png.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{settings}");
    assert!(settings["logo_url"]
        .as_str()
        .is_some_and(|value| value.starts_with("/api/project/logo?v=")));

    let target = std::env::temp_dir().join(format!(
        "conductor-project-storage-{}",
        uuid::Uuid::new_v4()
    ));
    let (status, migration) = app
        .put(
            "/api/settings/storage",
            Some(&token),
            serde_json::json!({
                "storage": {
                    "backend": "local",
                    "local": {"root": target.to_string_lossy()},
                    "s3": {
                        "bucket": "",
                        "region": "",
                        "endpoint": null,
                        "prefix": "",
                        "path_style": false
                    },
                    "azure_blob": {
                        "account": "",
                        "container": "",
                        "endpoint": null,
                        "prefix": ""
                    }
                },
                "migrate_existing": true
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{migration}");
    assert_eq!(migration["objects_copied"], 1);
    assert_eq!(migration["bytes_copied"], png.len());
    assert_eq!(migration["storage"]["backend"], "local");

    let (status, headers, observed) = app.get_bytes("/api/project/logo").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "image/png");
    assert_eq!(observed, png);

    let persisted = app.state.db.instance().storage_settings().await.unwrap();
    assert_eq!(persisted.local.root.as_deref(), target.to_str());
}

#[tokio::test]
async fn storage_settings_require_admin_and_complete_cloud_metadata() {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "object-storage-auth-test".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "storage-auth-admin@example.test".into(),
                admin_display_name: "Storage Auth Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let user_token = app.token_for_role(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;
    let incomplete_s3 = serde_json::json!({
        "storage": {
            "backend": "s3",
            "local": {"root": null},
            "s3": {"bucket": "", "region": "", "endpoint": null, "prefix": "", "path_style": false},
            "azure_blob": {"account": "", "container": "", "endpoint": null, "prefix": ""}
        },
        "migrate_existing": true
    });
    let (status, _) = app
        .put(
            "/api/settings/storage",
            Some(&user_token),
            incomplete_s3.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = app
        .put("/api/settings/storage", Some(&admin_token), incomplete_s3)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("S3 bucket"));
}

#[tokio::test]
async fn admin_migrates_objects_to_git_without_exposing_credentials() {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "git-storage-test".into(),
                display_name: Some("Git storage test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "git-storage-admin@example.test".into(),
                admin_display_name: "Git Storage Admin".into(),
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
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    png.extend_from_slice(b"git-storage-logo");
    let (status, _) = app
        .put_bytes("/api/settings/logo", Some(&token), "image/png", png)
        .await;
    assert_eq!(status, StatusCode::OK);
    let logo_key = app
        .state
        .db
        .instance()
        .logo_artifact()
        .await
        .unwrap()
        .unwrap()
        .key;

    let directory = std::env::temp_dir().join(format!(
        "conductor-project-git-storage-{}",
        uuid::Uuid::new_v4()
    ));
    let remote = directory.join("remote.git");
    std::fs::create_dir_all(&directory).unwrap();
    let initialized = Command::new("git")
        .args(["init", "--bare", remote.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(initialized.status.success());

    let storage_request = serde_json::json!({
        "storage": {
            "backend": "git",
            "local": {"root": null},
            "s3": {"bucket": "", "region": "", "endpoint": null, "prefix": "", "path_style": false},
            "azure_blob": {"account": "", "container": "", "endpoint": null, "prefix": ""},
            "git": {
                "repository_url": remote.to_string_lossy(),
                "branch": "resources",
                "prefix": "conductor-objects",
                "auth_mode": "environment",
                "username": null,
                "credential": null,
                "clear_credential": false,
                "credential_set": false
            }
        },
        "migrate_existing": true
    });
    let (status, migration) = app
        .put(
            "/api/settings/storage",
            Some(&token),
            storage_request.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{migration}");
    assert_eq!(migration["objects_copied"], 1);
    assert_eq!(migration["storage"]["backend"], "git");
    assert_eq!(migration["storage"]["git"]["credential_set"], false);
    assert!(migration["storage"]["git"].get("credential").is_none());
    assert!(migration["storage"]["git"]
        .get("clear_credential")
        .is_none());

    let verify = directory.join("verify");
    let cloned = Command::new("git")
        .args([
            "clone",
            "--branch",
            "resources",
            remote.to_str().unwrap(),
            verify.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        cloned.status.success(),
        "{}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    assert!(verify.join("conductor-objects").join(logo_key).is_file());

    let commit_count = Command::new("git")
        .args([
            "--git-dir",
            remote.to_str().unwrap(),
            "rev-list",
            "--count",
            "resources",
        ])
        .output()
        .unwrap();
    assert!(commit_count.status.success());
    let commit_count = commit_count.stdout;

    let (status, no_op) = app
        .put("/api/settings/storage", Some(&token), storage_request)
        .await;
    assert_eq!(status, StatusCode::OK, "{no_op}");
    assert_eq!(no_op["objects_copied"], 0);
    assert_eq!(no_op["bytes_copied"], 0);

    let unchanged_commit_count = Command::new("git")
        .args([
            "--git-dir",
            remote.to_str().unwrap(),
            "rev-list",
            "--count",
            "resources",
        ])
        .output()
        .unwrap();
    assert!(unchanged_commit_count.status.success());
    assert_eq!(unchanged_commit_count.stdout, commit_count);
}

#[tokio::test]
async fn project_data_policy_is_admin_only_and_updates_client_ingestion_gate() {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "data-policy-test".into(),
                display_name: Some("Data policy test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "policy-admin@example.test".into(),
                admin_display_name: "Policy Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let user_token = app.token_for_role(PrimaryRole::User).await;
    let admin_token = app.token_for(&admin).await;

    let (status, _) = app
        .put(
            "/api/settings/data-policy",
            Some(&user_token),
            serde_json::json!({"collection_level": "L0"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, settings) = app
        .put(
            "/api/settings/data-policy",
            Some(&admin_token),
            serde_json::json!({"collection_level": "L2"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{settings}");
    assert_eq!(settings["data_policy"]["collection_level"], "L2");
    assert_eq!(
        app.state.db.instance().collection_level().await.unwrap(),
        "L2"
    );

    let (status, body) = app
        .put(
            "/api/settings/data-policy",
            Some(&admin_token),
            serde_json::json!({"collection_level": "invalid"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

#[tokio::test]
async fn startup_externalizes_legacy_inline_resource_files() {
    let directory =
        std::env::temp_dir().join(format!("conductor-legacy-storage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let database_url = format!(
        "sqlite://{}?mode=rwc",
        directory.join("conductor.db").display()
    );
    let db = Db::connect(&database_url).await.unwrap();
    let (project, admin) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "legacy-storage-test".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "legacy-storage-admin@example.test".into(),
                admin_display_name: "Legacy Storage Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .unwrap();
    let resource_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now().to_rfc3339();
    let legacy_payload = serde_json::json!({
        "files": [{"path": "SKILL.md", "content": "legacy inline bytes"}]
    })
    .to_string();
    sqlx::query(
        r#"
        INSERT INTO resources (
            id, project_id, kind, slug, name, version, owner_user_id, visibility,
            status, payload, draft_revision, created_at, updated_at
        ) VALUES (?, ?, 'skill', 'legacy-skill', 'Legacy skill', '0.1.0', ?,
                  'shared', 'draft', ?, 0, ?, ?)
        "#,
    )
    .bind(resource_id.to_string())
    .bind(project.id.to_string())
    .bind(admin.id.to_string())
    .bind(&legacy_payload)
    .bind(&now)
    .bind(&now)
    .execute(db.pool())
    .await
    .unwrap();
    drop(db);

    let state = AppState::new(&database_url, RealtimeConfig::default())
        .await
        .unwrap();
    let row = sqlx::query("SELECT payload, draft_artifact_key FROM resources WHERE id = ?")
        .bind(resource_id.to_string())
        .fetch_one(state.db.pool())
        .await
        .unwrap();
    let payload: String = row.get("payload");
    assert!(!payload.contains("legacy inline bytes"));
    assert!(!payload.contains("\"content\""));
    let key: String = row.get("draft_artifact_key");
    assert!(!state.artifacts.read(&key).await.unwrap().is_empty());
}
