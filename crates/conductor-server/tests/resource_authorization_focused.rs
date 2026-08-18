mod support;

use std::io::{Cursor, Write};

use axum::http::StatusCode;
use conductor_domain::{
    CreateResourceRequest, DraftFile, PrimaryRole, ResourceKind, ResourceVisibility,
};
use conductor_server::core::resource_authoring::starter_files;
use serde_json::{json, Value};
use sqlx::Row;
use support::{test_app, TestApp};
use tokio::sync::broadcast::{error::TryRecvError, Receiver};
use uuid::Uuid;

type VersionSnapshot = (String, String, Option<String>, Option<String>, String);
type VersionEventSnapshot = (String, String, String, String, Option<String>, i64);
type AccessRuleSnapshot = (String, String, String, String);
type ChangeSnapshot = (i64, String, Option<String>, Option<String>);

#[derive(Debug, PartialEq)]
struct ResourceSnapshot {
    status: String,
    highest_semver: Option<String>,
    draft_revision: i64,
    draft_artifact_key: String,
    draft_content_sha256: String,
    payload: String,
    versions: Vec<VersionSnapshot>,
    version_events: Vec<VersionEventSnapshot>,
    access_rules: Vec<AccessRuleSnapshot>,
    release_channels: Vec<(String, String)>,
    changes: Vec<ChangeSnapshot>,
    beta_member_count: i64,
}

#[tokio::test]
async fn owner_sensitive_resource_handlers_allow_owner_hide_peer_and_reject_cross_project() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let owner = app.seed_user(PrimaryRole::Contribute).await;
    let peer = app.seed_user(PrimaryRole::Contribute).await;
    let admin_token = app.token_for(&admin).await;
    let owner_token = app.token_for(&owner).await;
    let peer_token = app.token_for(&peer).await;

    let slug = "focused-owned-agent";
    let files = starter_files(ResourceKind::Agent, slug, "Focused owned agent");
    let resource_id =
        create_resource(&app, &owner_token, ResourceKind::Agent, slug, files.clone()).await;
    let resource_path = format!("/api/resources/{resource_id}");
    let access_path = format!("{resource_path}/access");
    let validate_path = format!("{resource_path}/draft/validate");
    let import_path = format!("{resource_path}/draft/import?draft_revision=1");
    let feedback_path = format!("{resource_path}/feedback");
    let monitoring_path = format!("{resource_path}/monitoring");
    let release_path = format!("{resource_path}/release");

    assert_eq!(
        app.get(&access_path, Some(&owner_token)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        app.get(&access_path, Some(&peer_token)).await.0,
        StatusCode::NOT_FOUND
    );
    let before_peer_access = resource_snapshot(&app, resource_id).await;
    assert_eq!(
        app.put(&access_path, Some(&peer_token), empty_access_policy(),)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        resource_snapshot(&app, resource_id).await,
        before_peer_access,
        "denied peer access-policy update changed resource state"
    );
    assert_eq!(
        app.put(&access_path, Some(&owner_token), empty_access_policy(),)
            .await
            .0,
        StatusCode::OK
    );

    let source = files
        .iter()
        .find(|file| file.path == format!("{slug}.md"))
        .expect("agent source");
    let save_path = format!("{resource_path}/draft/files/{}", source.path);
    let save_body = json!({
        "content": format!("{}\n", source.content),
        "draft_revision": 0
    });
    let before_peer_draft = resource_snapshot(&app, resource_id).await;
    assert_eq!(
        app.put(&save_path, Some(&peer_token), save_body.clone())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        resource_snapshot(&app, resource_id).await,
        before_peer_draft
    );
    let (status, saved) = app.put(&save_path, Some(&owner_token), save_body).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["revision"], 1);

    let archive_bytes = archive(&files);
    assert_eq!(
        app.post_bytes(
            &import_path,
            Some(&peer_token),
            "application/zip",
            archive_bytes.clone(),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, imported) = app
        .post_bytes(
            &import_path,
            Some(&owner_token),
            "application/zip",
            archive_bytes,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["tree"]["revision"], 2);
    assert_eq!(imported["validation"]["valid"], true);

    let (status, validation) = app
        .post(&validate_path, Some(&owner_token), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{validation}");
    assert_eq!(validation["valid"], true);
    assert_eq!(
        app.post(&validate_path, Some(&peer_token), json!({}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let (status, feedback) = app.get(&feedback_path, Some(&owner_token)).await;
    assert_eq!(status, StatusCode::OK, "{feedback}");
    assert_eq!(feedback.as_array().map(Vec::len), Some(0));
    assert_eq!(
        app.get(&feedback_path, Some(&peer_token)).await.0,
        StatusCode::NOT_FOUND
    );

    let (status, monitoring) = app.get(&monitoring_path, Some(&owner_token)).await;
    assert_eq!(status, StatusCode::OK, "{monitoring}");
    assert_eq!(monitoring["members"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        app.get(&monitoring_path, Some(&peer_token)).await.0,
        StatusCode::NOT_FOUND
    );

    let mut realtime = app.state.realtime.subscribe();
    let before_peer_lifecycle = resource_snapshot(&app, resource_id).await;
    assert_eq!(
        app.post(&release_path, Some(&peer_token), release_body(2))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post(
            &format!("{resource_path}/archive"),
            Some(&peer_token),
            json!({}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        resource_snapshot(&app, resource_id).await,
        before_peer_lifecycle
    );
    assert_no_realtime(&mut realtime);

    let first_version = release(&app, &owner_token, resource_id, 2).await;
    let _second_version = release(&app, &owner_token, resource_id, 3).await;
    let before_peer_version_lifecycle = resource_snapshot(&app, resource_id).await;
    assert_eq!(
        app.post(
            &format!("{resource_path}/versions/{first_version}/deprecate"),
            Some(&peer_token),
            json!({"reason": "peer must not deprecate"}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post(
            &format!("{resource_path}/versions/{first_version}/restore-to-draft"),
            Some(&peer_token),
            json!({"draft_revision": 4, "confirm_deprecated": true}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        resource_snapshot(&app, resource_id).await,
        before_peer_version_lifecycle
    );

    let (status, deprecated) = app
        .post(
            &format!("{resource_path}/versions/{first_version}/deprecate"),
            Some(&owner_token),
            json!({"reason": "superseded"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{deprecated}");
    let (status, restored) = app
        .post(
            &format!("{resource_path}/versions/{first_version}/restore-to-draft"),
            Some(&owner_token),
            json!({"draft_revision": 4, "confirm_deprecated": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["revision"], 5);
    assert_eq!(
        app.post(
            &format!("{resource_path}/archive"),
            Some(&owner_token),
            json!({}),
        )
        .await
        .0,
        StatusCode::OK
    );

    let cross_slug = "focused-cross-project-agent";
    let cross_files = starter_files(
        ResourceKind::Agent,
        cross_slug,
        "Focused cross project agent",
    );
    let cross_id = create_resource(
        &app,
        &owner_token,
        ResourceKind::Agent,
        cross_slug,
        cross_files.clone(),
    )
    .await;
    let cross_version = release(&app, &owner_token, cross_id, 0).await;
    let _cross_active_version = release(&app, &owner_token, cross_id, 1).await;
    move_resource_to_foreign_project(&app, cross_id).await;
    let cross_path = format!("/api/resources/{cross_id}");
    let cross_save_path = format!("{cross_path}/draft/files/{cross_slug}.md");
    let before_cross_project = resource_snapshot(&app, cross_id).await;
    let mut cross_realtime = app.state.realtime.subscribe();

    assert_eq!(
        app.get(&format!("{cross_path}/access"), Some(&admin_token))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.put(
            &format!("{cross_path}/access"),
            Some(&admin_token),
            empty_access_policy(),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.put(
            &cross_save_path,
            Some(&admin_token),
            json!({"content": "cross project", "draft_revision": 2}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post_bytes(
            &format!("{cross_path}/draft/import?draft_revision=2"),
            Some(&admin_token),
            "application/zip",
            archive(&cross_files),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post(
            &format!("{cross_path}/draft/validate"),
            Some(&admin_token),
            json!({}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for suffix in ["feedback", "monitoring"] {
        assert_eq!(
            app.get(&format!("{cross_path}/{suffix}"), Some(&admin_token))
                .await
                .0,
            StatusCode::NOT_FOUND,
            "cross-project {suffix}"
        );
    }
    assert_eq!(
        app.post(
            &format!("{cross_path}/release"),
            Some(&admin_token),
            release_body(2),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post(
            &format!("{cross_path}/versions/{cross_version}/deprecate"),
            Some(&admin_token),
            json!({"reason": "cross project"}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post(
            &format!("{cross_path}/versions/{cross_version}/restore-to-draft"),
            Some(&admin_token),
            json!({"draft_revision": 2, "confirm_deprecated": true}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post(
            &format!("{cross_path}/archive"),
            Some(&admin_token),
            json!({}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        resource_snapshot(&app, cross_id).await,
        before_cross_project
    );
    assert_no_realtime(&mut cross_realtime);
}

#[tokio::test]
async fn resource_kind_matrix_proves_allowed_lifecycle_and_denied_zero_side_effects() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let contributor = app.seed_user(PrimaryRole::Contribute).await;
    let admin_token = app.token_for(&admin).await;
    let contributor_token = app.token_for(&contributor).await;

    for kind in [ResourceKind::Agent, ResourceKind::Skill] {
        let slug = format!("contributor-{}", kind.as_str());
        let resource_id = create_resource(
            &app,
            &contributor_token,
            kind,
            &slug,
            starter_files(kind, &slug, &format!("Contributor {kind:?}")),
        )
        .await;
        let first_version = release(&app, &contributor_token, resource_id, 0).await;
        let _second_version = release(&app, &contributor_token, resource_id, 1).await;
        let path = format!("/api/resources/{resource_id}");

        let (status, deprecated) = app
            .post(
                &format!("{path}/versions/{first_version}/deprecate"),
                Some(&contributor_token),
                json!({"reason": "superseded"}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{kind:?}: {deprecated}");
        let (status, restored) = app
            .post(
                &format!("{path}/versions/{first_version}/restore-to-draft"),
                Some(&contributor_token),
                json!({"draft_revision": 2, "confirm_deprecated": true}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{kind:?}: {restored}");
        assert_eq!(restored["revision"], 3);
        assert_eq!(
            app.post(
                &format!("{path}/archive"),
                Some(&contributor_token),
                json!({}),
            )
            .await
            .0,
            StatusCode::OK,
            "Contributor owner should archive {kind:?}"
        );
    }

    for kind in [
        ResourceKind::Plugin,
        ResourceKind::Workflow,
        ResourceKind::Command,
    ] {
        let slug = format!("restricted-{}", kind.as_str());
        let resource_id = create_resource(
            &app,
            &contributor_token,
            kind,
            &slug,
            starter_files(kind, &slug, &format!("Restricted {kind:?}")),
        )
        .await;
        let first_version = release(&app, &admin_token, resource_id, 0).await;
        let _second_version = release(&app, &admin_token, resource_id, 1).await;
        let path = format!("/api/resources/{resource_id}");
        let before_denials = resource_snapshot(&app, resource_id).await;
        let mut realtime = app.state.realtime.subscribe();

        assert_denied_release_without_artifact_write(
            &app,
            &path,
            &contributor_token,
            release_body(2),
            &format!("Contributor restricted Published release for {kind:?}"),
        )
        .await;
        assert_denied_release_without_artifact_write(
            &app,
            &path,
            &contributor_token,
            beta_release_body(2, contributor.id),
            &format!("Contributor restricted Beta release for {kind:?}"),
        )
        .await;
        assert_eq!(
            app.post(
                &format!("{path}/versions/{first_version}/deprecate"),
                Some(&contributor_token),
                json!({"reason": "must remain unchanged"}),
            )
            .await
            .0,
            StatusCode::FORBIDDEN,
            "Contributor restricted deprecation for {kind:?}"
        );
        assert_eq!(
            app.post(
                &format!("{path}/versions/{first_version}/restore-to-draft"),
                Some(&contributor_token),
                json!({"draft_revision": 2, "confirm_deprecated": true}),
            )
            .await
            .0,
            StatusCode::FORBIDDEN,
            "Contributor restricted restore for {kind:?}"
        );
        assert_eq!(
            app.post(
                &format!("{path}/archive"),
                Some(&contributor_token),
                json!({}),
            )
            .await
            .0,
            StatusCode::FORBIDDEN,
            "Contributor restricted archive for {kind:?}"
        );
        assert_eq!(
            resource_snapshot(&app, resource_id).await,
            before_denials,
            "denied restricted operations changed {kind:?} storage/artifact bindings"
        );
        assert_no_realtime(&mut realtime);

        let (status, deprecated) = app
            .post(
                &format!("{path}/versions/{first_version}/deprecate"),
                Some(&admin_token),
                json!({"reason": "admin restricted lifecycle"}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "Admin {kind:?}: {deprecated}");
        let (status, restored) = app
            .post(
                &format!("{path}/versions/{first_version}/restore-to-draft"),
                Some(&admin_token),
                json!({"draft_revision": 2, "confirm_deprecated": true}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "Admin {kind:?}: {restored}");
        assert_eq!(restored["revision"], 3);
        assert_eq!(
            app.post(&format!("{path}/archive"), Some(&admin_token), json!({}),)
                .await
                .0,
            StatusCode::OK,
            "Admin should archive restricted {kind:?}"
        );
    }
}

async fn create_resource(
    app: &TestApp,
    token: &str,
    kind: ResourceKind,
    slug: &str,
    files: Vec<DraftFile>,
) -> Uuid {
    let request = CreateResourceRequest {
        kind,
        slug: slug.into(),
        name: slug.replace('-', " "),
        description: None,
        version: "0.1.0".into(),
        visibility: ResourceVisibility::Shared,
        payload: json!({"files": files}),
        changelog: None,
    };
    let (status, resource) = app
        .post(
            "/api/resources",
            Some(token),
            serde_json::to_value(request).expect("serialize resource request"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{kind:?}: {resource}");
    Uuid::parse_str(resource["id"].as_str().expect("resource id")).expect("UUID resource id")
}

async fn release(app: &TestApp, token: &str, resource_id: Uuid, revision: u64) -> Uuid {
    let (status, released) = app
        .post(
            &format!("/api/resources/{resource_id}/release"),
            Some(token),
            release_body(revision),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{released}");
    Uuid::parse_str(released["version_id"].as_str().expect("version id")).expect("UUID version id")
}

async fn assert_denied_release_without_artifact_write(
    app: &TestApp,
    resource_path: &str,
    token: &str,
    body: Value,
    context: &str,
) {
    let artifacts_before = app.artifact_snapshot();
    let object_count_before = artifacts_before.len();
    let (status, response) = app
        .post(&format!("{resource_path}/release"), Some(token), body)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{context}: {response}");

    let artifacts_after = app.artifact_snapshot();
    assert_eq!(
        artifacts_after.len(),
        object_count_before,
        "{context} changed the physical artifact count"
    );
    assert_eq!(
        artifacts_after, artifacts_before,
        "{context} created or replaced a physical artifact"
    );
}

fn release_body(draft_revision: u64) -> Value {
    json!({
        "channel": "published",
        "version_mode": "auto",
        "manual_version": null,
        "draft_revision": draft_revision,
        "changelog": "Focused authorization proof",
        "beta_member_ids": [],
        "minimum_evoflux_version": null
    })
}

fn beta_release_body(draft_revision: u64, beta_member_id: Uuid) -> Value {
    json!({
        "channel": "beta",
        "version_mode": "auto",
        "manual_version": null,
        "draft_revision": draft_revision,
        "changelog": "Focused Beta authorization proof",
        "beta_member_ids": [beta_member_id],
        "minimum_evoflux_version": null
    })
}

fn empty_access_policy() -> Value {
    json!({
        "all_members": false,
        "primary_roles": [],
        "sub_role_ids": [],
        "tag_ids": [],
        "member_ids": []
    })
}

fn archive(files: &[DraftFile]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for file in files {
        writer.start_file(&file.path, options).expect("zip entry");
        writer
            .write_all(file.content.as_bytes())
            .expect("zip content");
    }
    writer.finish().expect("finish ZIP").into_inner()
}

async fn move_resource_to_foreign_project(app: &TestApp, resource_id: Uuid) {
    let mut connection = app
        .state
        .db
        .pool()
        .acquire()
        .await
        .expect("acquire database connection");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .expect("disable foreign keys for cross-project fixture");
    let foreign_project_id = Uuid::new_v4().to_string();
    for table in [
        "resource_versions",
        "resource_access_rules",
        "resource_release_channels",
        "resource_beta_members",
        "resource_changes",
        "resource_version_events",
    ] {
        let statement = format!("UPDATE {table} SET project_id = ? WHERE resource_id = ?");
        sqlx::query(&statement)
            .bind(&foreign_project_id)
            .bind(resource_id.to_string())
            .execute(&mut *connection)
            .await
            .expect("move project-scoped resource child rows");
    }
    sqlx::query("UPDATE resources SET project_id = ? WHERE id = ?")
        .bind(foreign_project_id)
        .bind(resource_id.to_string())
        .execute(&mut *connection)
        .await
        .expect("move resource to foreign project");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("restore foreign keys");
}

async fn resource_snapshot(app: &TestApp, resource_id: Uuid) -> ResourceSnapshot {
    let resource = sqlx::query(
        "SELECT status, highest_semver, draft_revision, draft_artifact_key, \
         draft_content_sha256, payload FROM resources WHERE id = ?",
    )
    .bind(resource_id.to_string())
    .fetch_one(app.state.db.pool())
    .await
    .expect("resource snapshot");
    let versions = sqlx::query(
        "SELECT id, status, deprecation_reason, artifact_key, content_sha256 \
         FROM resource_versions WHERE resource_id = ? ORDER BY id",
    )
    .bind(resource_id.to_string())
    .fetch_all(app.state.db.pool())
    .await
    .expect("version snapshot")
    .into_iter()
    .map(|row| {
        (
            row.get("id"),
            row.get("status"),
            row.get("deprecation_reason"),
            row.get("artifact_key"),
            row.get("content_sha256"),
        )
    })
    .collect();
    let version_events = sqlx::query(
        "SELECT id, version_id, action, actor_id, reason, confirmed_deprecated \
         FROM resource_version_events WHERE resource_id = ? ORDER BY id",
    )
    .bind(resource_id.to_string())
    .fetch_all(app.state.db.pool())
    .await
    .expect("version event snapshot")
    .into_iter()
    .map(|row| {
        (
            row.get("id"),
            row.get("version_id"),
            row.get("action"),
            row.get("actor_id"),
            row.get("reason"),
            row.get("confirmed_deprecated"),
        )
    })
    .collect();
    let access_rules = sqlx::query(
        "SELECT project_id, subject_type, subject_id, effect FROM resource_access_rules \
         WHERE resource_id = ? ORDER BY project_id, subject_type, subject_id, effect",
    )
    .bind(resource_id.to_string())
    .fetch_all(app.state.db.pool())
    .await
    .expect("access rule snapshot")
    .into_iter()
    .map(|row| {
        (
            row.get("project_id"),
            row.get("subject_type"),
            row.get("subject_id"),
            row.get("effect"),
        )
    })
    .collect();
    let release_channels = sqlx::query(
        "SELECT channel, version_id FROM resource_release_channels \
         WHERE resource_id = ? ORDER BY channel",
    )
    .bind(resource_id.to_string())
    .fetch_all(app.state.db.pool())
    .await
    .expect("release channel snapshot")
    .into_iter()
    .map(|row| (row.get("channel"), row.get("version_id")))
    .collect();
    let changes = sqlx::query(
        "SELECT sequence, change_kind, version_id, effective_user_id FROM resource_changes \
         WHERE resource_id = ? ORDER BY sequence",
    )
    .bind(resource_id.to_string())
    .fetch_all(app.state.db.pool())
    .await
    .expect("change snapshot")
    .into_iter()
    .map(|row| {
        (
            row.get("sequence"),
            row.get("change_kind"),
            row.get("version_id"),
            row.get("effective_user_id"),
        )
    })
    .collect();
    let beta_member_count =
        sqlx::query_scalar("SELECT COUNT(*) FROM resource_beta_members WHERE resource_id = ?")
            .bind(resource_id.to_string())
            .fetch_one(app.state.db.pool())
            .await
            .expect("beta member snapshot");

    ResourceSnapshot {
        status: resource.get("status"),
        highest_semver: resource.get("highest_semver"),
        draft_revision: resource.get("draft_revision"),
        draft_artifact_key: resource.get("draft_artifact_key"),
        draft_content_sha256: resource.get("draft_content_sha256"),
        payload: resource.get("payload"),
        versions,
        version_events,
        access_rules,
        release_channels,
        changes,
        beta_member_count,
    }
}

fn assert_no_realtime(receiver: &mut Receiver<conductor_server::http::realtime::RealtimeMessage>) {
    assert!(
        matches!(receiver.try_recv(), Err(TryRecvError::Empty)),
        "denied resource operation emitted a realtime signal"
    );
}
