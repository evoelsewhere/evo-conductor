mod support;

use axum::http::StatusCode;
use conductor_auth::{hash_password_async, verify_password_async};
use conductor_domain::{
    CreateResourceRequest, PrimaryRole, ResourceKind, ResourceVisibility, SecretScope,
};
use conductor_server::core::resource_authoring::starter_files;
use serde_json::{json, Value};
use sqlx::Row;
use support::{test_app, TestApp};
use uuid::Uuid;

#[tokio::test]
async fn password_change_persists_the_new_password_and_rotates_the_session() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let seeded = app.seed_user(PrimaryRole::User).await;
    let current_password = "current-password-123";
    let new_password = "new-password-456789";
    let current_hash = hash_password_async(current_password.to_string())
        .await
        .expect("hash current password");
    app.state
        .db
        .users()
        .set_password(seeded.id, &current_hash, false)
        .await
        .expect("set known current password");
    let user = app
        .state
        .db
        .users()
        .find_by_id(seeded.id)
        .await
        .expect("load password-change user")
        .expect("password-change user exists");
    let old_token = app.token_for(&user).await;

    let (status, session) = app
        .post(
            "/api/auth/change-password",
            Some(&old_token),
            json!({
                "current_password": current_password,
                "new_password": new_password
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(session["user"]["id"], user.id.to_string());
    assert_eq!(session["user"]["must_change_password"], false);
    let new_token = session["token"].as_str().expect("rotated session token");
    assert_ne!(new_token, old_token);

    assert_eq!(
        app.get("/api/auth/me", Some(&old_token)).await.0,
        StatusCode::UNAUTHORIZED,
        "the password update must invalidate the prior session version"
    );
    let (status, me) = app.get("/api/auth/me", Some(new_token)).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["id"], user.id.to_string());

    let (_, stored_hash) = app
        .state
        .db
        .users()
        .find_by_email(&user.email)
        .await
        .expect("read updated password")
        .expect("updated user exists");
    let stored_hash = stored_hash.expect("local account password hash");
    assert!(
        verify_password_async(new_password.to_string(), stored_hash.clone())
            .await
            .expect("verify new password")
    );
    assert!(
        !verify_password_async(current_password.to_string(), stored_hash)
            .await
            .expect("reject old password")
    );
}

#[tokio::test]
async fn admin_network_and_sso_updates_persist_safe_response_state() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let admin_token = app.token_for(&admin).await;

    let (status, network) = app
        .put(
            "/api/settings/network",
            Some(&admin_token),
            json!({
                "bind_host": "0.0.0.0",
                "bind_port": 4811,
                "public_url": "https://conductor.example.test",
                "realtime": {
                    "max_connections": 10001,
                    "max_connections_per_secret": 7,
                    "heartbeat_seconds": 45
                }
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{network}");
    assert_eq!(network["bind_host"], "0.0.0.0");
    assert_eq!(network["bind_port"], 4811);
    assert_eq!(network["public_url"], "https://conductor.example.test");
    assert_eq!(network["realtime"]["max_connections"], 10001);
    assert_eq!(network["realtime"]["max_connections_per_secret"], 7);
    assert_eq!(network["realtime"]["heartbeat_seconds"], 45);

    let persisted = app
        .state
        .db
        .instance()
        .get()
        .await
        .expect("read persisted network settings")
        .expect("project identity");
    assert_eq!(persisted.bind_host, "0.0.0.0");
    assert_eq!(persisted.bind_port, 4811);
    assert_eq!(
        persisted.public_url.as_deref(),
        Some("https://conductor.example.test")
    );
    let live_realtime = app.state.realtime.config();
    assert_eq!(live_realtime.max_connections, 10001);
    assert_eq!(live_realtime.max_connections_per_secret, 7);
    assert_eq!(live_realtime.heartbeat_seconds, 45);
    let persisted_realtime = app
        .state
        .db
        .instance()
        .network_overrides()
        .await
        .expect("read persisted realtime overrides");
    assert_eq!(persisted_realtime.realtime_max_connections, Some(10001));
    assert_eq!(persisted_realtime.realtime_max_per_secret, Some(7));
    assert_eq!(persisted_realtime.realtime_heartbeat_seconds, Some(45));

    let client_secret = "sso-secret-must-never-be-returned";
    let (status, updated_sso) = app
        .put(
            "/api/sso",
            Some(&admin_token),
            json!({
                "enabled": true,
                "provider": "oidc",
                "issuer_url": "https://issuer.example.test",
                "client_id": "evo-conductor-test",
                "client_secret": client_secret,
                "redirect_uri": "http://127.0.0.1:4700/api/auth/sso/callback",
                "scopes": ["openid", "profile", "email"]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated_sso}");
    assert_eq!(updated_sso["enabled"], true);
    assert_eq!(updated_sso["provider"], "oidc");
    assert_eq!(updated_sso["client_secret_set"], true);
    assert!(updated_sso.get("client_secret").is_none());
    assert!(!updated_sso.to_string().contains(client_secret));

    let (status, read_sso) = app.get("/api/sso", Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{read_sso}");
    assert_eq!(read_sso, updated_sso);
    assert!(read_sso.get("client_secret").is_none());
    assert!(!read_sso.to_string().contains(client_secret));
}

#[tokio::test]
async fn member_lifecycle_and_self_connection_token_routes_persist_meaningful_state() {
    let app = test_app().await;
    app.seed_project_identity().await;
    mark_setup_completed(&app).await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let admin_token = app.token_for(&admin).await;

    let invited_email = format!("invited-{}@example.test", Uuid::new_v4().simple());
    let (status, created) = app
        .post(
            "/api/members",
            Some(&admin_token),
            json!({
                "email": invited_email,
                "display_name": "Invited compatibility member",
                "primary_role": "user",
                "sub_role_ids": [],
                "tag_ids": []
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["user"]["status"], "invited");
    assert_eq!(created["user"]["must_change_password"], true);
    let invited_id = Uuid::parse_str(created["user"]["id"].as_str().expect("invited member id"))
        .expect("UUID invited member id");
    let invite_password = created["temporary_password"]
        .as_str()
        .expect("one-time invite password");
    let (_, invite_hash) = app
        .state
        .db
        .users()
        .find_by_email(&invited_email)
        .await
        .expect("read invited member")
        .expect("invited member exists");
    assert!(verify_password_async(
        invite_password.to_string(),
        invite_hash.expect("invite password hash")
    )
    .await
    .expect("verify invite password"));

    let (status, count) = app
        .get("/api/members/pending/count", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{count}");
    assert_eq!(
        count["count"], 0,
        "password invitations are not SSO-pending"
    );

    let pending = app
        .state
        .db
        .users()
        .create_pending_sso(
            "https://issuer.example.test",
            &format!("subject-{}", Uuid::new_v4().simple()),
            &format!("pending-{}@example.test", Uuid::new_v4().simple()),
            "Pending SSO member",
        )
        .await
        .expect("seed the post-IdP pending state");
    let (status, count) = app
        .get("/api/members/pending/count", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{count}");
    assert_eq!(count["count"], 1);
    let (status, approved_pending) = app
        .post(
            &format!("/api/members/{}/approve", pending.id),
            Some(&admin_token),
            json!({
                "primary_role": "contribute",
                "sub_role_ids": [],
                "tag_ids": []
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{approved_pending}");
    assert_eq!(approved_pending["status"], "active");
    assert_eq!(approved_pending["primary_role"], "contribute");
    let (status, count) = app
        .get("/api/members/pending/count", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{count}");
    assert_eq!(count["count"], 0);

    let (status, approved_invite) = app
        .post(
            &format!("/api/members/{invited_id}/approve"),
            Some(&admin_token),
            json!({
                "primary_role": "user",
                "sub_role_ids": [],
                "tag_ids": []
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{approved_invite}");
    assert_eq!(approved_invite["status"], "active");

    let (status, reset) = app
        .post(
            &format!("/api/members/{invited_id}/reset-password"),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{reset}");
    let reset_password = reset["temporary_password"]
        .as_str()
        .expect("reset temporary password");
    assert_ne!(reset_password, invite_password);
    let reset_member = app
        .state
        .db
        .users()
        .find_by_id(invited_id)
        .await
        .expect("read reset member")
        .expect("reset member exists");
    assert!(reset_member.must_change_password);
    let (_, reset_hash) = app
        .state
        .db
        .users()
        .find_by_email(&invited_email)
        .await
        .expect("read reset password")
        .expect("reset member exists");
    assert!(verify_password_async(
        reset_password.to_string(),
        reset_hash.expect("reset password hash")
    )
    .await
    .expect("verify reset password"));

    let (status, login) = app
        .post(
            "/api/auth/login",
            None,
            json!({"email": invited_email, "password": reset_password}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    assert_eq!(login["user"]["id"], invited_id.to_string());
    assert_eq!(login["user"]["must_change_password"], true);
    let member_token = login["token"].as_str().expect("member browser token");

    let (status, self_created) = app
        .post(
            "/api/secrets",
            Some(member_token),
            json!({
                "name": "Self API token",
                "scopes": [SecretScope::SubscribeResources]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{self_created}");
    assert!(self_created["token"]
        .as_str()
        .is_some_and(|token| token.starts_with("evc_")));
    assert!(self_created["secret"].get("token_hash").is_none());

    let (status, self_list) = app.get("/api/secrets", Some(member_token)).await;
    assert_eq!(status, StatusCode::OK, "{self_list}");
    assert_eq!(self_list.as_array().map(Vec::len), Some(1));
    assert_safe_secret_metadata(&self_list[0]);

    let (status, member_created) = app
        .post(
            &format!("/api/members/{invited_id}/secrets"),
            Some(member_token),
            json!({
                "name": "Self member-path token",
                "scopes": [SecretScope::ReportTelemetry]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{member_created}");
    assert!(member_created["token"]
        .as_str()
        .is_some_and(|token| token.starts_with("evc_")));

    let (status, member_list) = app
        .get(
            &format!("/api/members/{invited_id}/secrets"),
            Some(member_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{member_list}");
    assert_eq!(member_list.as_array().map(Vec::len), Some(2));
    for secret in member_list.as_array().expect("member token list") {
        assert_safe_secret_metadata(secret);
    }
    assert_eq!(
        app.state
            .db
            .secrets()
            .list_for_user(invited_id)
            .await
            .expect("persisted member secrets")
            .len(),
        2
    );
}

#[tokio::test]
async fn taxonomy_sub_role_and_tag_crud_returns_and_persists_updates() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let admin_token = app.token_for_role(PrimaryRole::Admin).await;

    let (status, sub_role) = app
        .post(
            "/api/sub-roles",
            Some(&admin_token),
            json!({
                "slug": "release-engineering",
                "name": "Release Engineering",
                "description": "Initial description",
                "color": "#112233"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{sub_role}");
    let sub_role_id = sub_role["id"].as_str().expect("sub-role id");
    let (status, updated_sub_role) = app
        .patch(
            &format!("/api/sub-roles/{sub_role_id}"),
            Some(&admin_token),
            json!({
                "name": "Release Platform",
                "description": "Updated description",
                "color": "#A1B2C3"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated_sub_role}");
    assert_eq!(updated_sub_role["slug"], "release-engineering");
    assert_eq!(updated_sub_role["name"], "Release Platform");
    assert_eq!(updated_sub_role["description"], "Updated description");
    assert_eq!(updated_sub_role["color"], "#A1B2C3");

    let (status, tag) = app
        .post(
            "/api/tags",
            Some(&admin_token),
            json!({
                "slug": "regulated",
                "name": "Regulated",
                "description": null,
                "color": "#445566"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{tag}");
    let tag_id = tag["id"].as_str().expect("tag id");
    let (status, updated_tag) = app
        .patch(
            &format!("/api/tags/{tag_id}"),
            Some(&admin_token),
            json!({
                "name": "Regulated Workloads",
                "description": "Requires compliance review",
                "color": "#778899"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated_tag}");
    assert_eq!(updated_tag["slug"], "regulated");
    assert_eq!(updated_tag["name"], "Regulated Workloads");
    assert_eq!(updated_tag["description"], "Requires compliance review");
    assert_eq!(updated_tag["color"], "#778899");

    let (status, sub_roles) = app.get("/api/sub-roles", Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{sub_roles}");
    assert!(sub_roles.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["id"] == sub_role_id && item["name"] == "Release Platform")
    }));
    let (status, tags) = app.get("/api/tags", Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{tags}");
    assert!(tags.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["id"] == tag_id && item["name"] == "Regulated Workloads")
    }));

    let (status, deleted_sub_role) = app
        .delete(
            &format!("/api/sub-roles/{sub_role_id}"),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{deleted_sub_role}");
    assert_eq!(deleted_sub_role["deleted"], true);
    let (status, deleted_tag) = app
        .delete(
            &format!("/api/tags/{tag_id}"),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{deleted_tag}");
    assert_eq!(deleted_tag["deleted"], true);
    assert!(app
        .state
        .db
        .roles()
        .list_sub_roles()
        .await
        .expect("list persisted sub-roles")
        .is_empty());
    assert!(app
        .state
        .db
        .roles()
        .list_tags()
        .await
        .expect("list persisted tags")
        .is_empty());
}

#[tokio::test]
async fn authoring_metadata_access_and_resource_usage_have_real_success_contracts() {
    let app = test_app().await;
    app.seed_project_identity().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let admin_token = app.token_for(&admin).await;

    let (status, guide) = app
        .get("/api/resources/guides/agent", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{guide}");
    assert_eq!(guide["kind"], "agent");
    assert_eq!(guide["schema_version"], 1);
    assert_eq!(guide["title"], "EvoFlux Agent");
    assert_eq!(guide["required_entries"], json!(["<slug>.md"]));
    assert!(guide["max_files"].as_u64().is_some_and(|value| value > 0));
    assert!(guide["max_editable_file_bytes"]
        .as_u64()
        .is_some_and(|value| value > 0));

    let (status, template) = app
        .get("/api/resources/templates/agent", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "{template}");
    assert_eq!(template["resource_id"], Uuid::nil().to_string());
    assert_eq!(template["revision"], 0);
    let template_files = template["files"].as_array().expect("template files");
    assert!(template_files.iter().any(|file| {
        file["path"] == "new-resource.md"
            && file["content"]
                .as_str()
                .is_some_and(|content| content.contains("New resource"))
    }));

    let slug = format!("usage-proof-{}", Uuid::new_v4().simple());
    let create_request = CreateResourceRequest {
        kind: ResourceKind::Agent,
        slug: slug.clone(),
        name: "Usage proof agent".into(),
        description: Some("Released for a valid usage event".into()),
        version: "0.1.0".into(),
        visibility: ResourceVisibility::Shared,
        payload: json!({
            "files": starter_files(ResourceKind::Agent, &slug, "Usage proof agent")
        }),
        changelog: None,
    };
    let (status, resource) = app
        .post(
            "/api/resources",
            Some(&admin_token),
            serde_json::to_value(create_request).expect("serialize resource request"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resource}");
    let resource_id =
        Uuid::parse_str(resource["id"].as_str().expect("resource id")).expect("UUID resource id");

    let (status, access) = app
        .get(
            &format!("/api/resources/{resource_id}/access"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{access}");
    assert_eq!(
        access,
        json!({
            "all_members": false,
            "primary_roles": [],
            "sub_role_ids": [],
            "tag_ids": [],
            "member_ids": []
        })
    );

    let (status, released) = app
        .post(
            &format!("/api/resources/{resource_id}/release"),
            Some(&admin_token),
            json!({
                "channel": "published",
                "version_mode": "auto",
                "manual_version": null,
                "draft_revision": 0,
                "changelog": "Usage compatibility proof",
                "beta_member_ids": [],
                "minimum_evoflux_version": null
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{released}");
    assert_eq!(released["resource_id"], resource_id.to_string());
    assert_eq!(released["version"], "0.1.0");
    assert_eq!(released["channel"], "published");

    let (status, secret) = app
        .post(
            "/api/secrets",
            Some(&admin_token),
            json!({
                "name": "Usage ingestion token",
                "scopes": [SecretScope::ReportTelemetry]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{secret}");
    let connection_token = secret["token"].as_str().expect("raw connection token");
    let event_id = Uuid::new_v4();
    let event = json!({
        "events": [{
            "event_id": event_id,
            "resource_id": resource_id,
            "resource_version": "0.1.0",
            "session_id": "opaque-session-proof",
            "outcome": "success",
            "duration_ms": 1450,
            "tokens_in": 123,
            "tokens_out": 45,
            "occurred_at": chrono::Utc::now().to_rfc3339()
        }]
    });
    let (status, accepted) = app
        .post(
            "/api/v1/usage/resources",
            Some(connection_token),
            event.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["accepted"], 1);
    assert_eq!(accepted["duplicates"], 0);
    assert_eq!(accepted["rejected"], 0);
    assert_eq!(accepted["rejections"], json!([]));

    let (status, duplicate) = app
        .post("/api/v1/usage/resources", Some(connection_token), event)
        .await;
    assert_eq!(status, StatusCode::OK, "{duplicate}");
    assert_eq!(duplicate["accepted"], 0);
    assert_eq!(duplicate["duplicates"], 1);
    assert_eq!(duplicate["rejected"], 0);

    let persisted = sqlx::query(
        "SELECT resource_id, resource_version, user_id, session_id, outcome, \
                duration_ms, tokens_in, tokens_out \
         FROM resource_usage_events WHERE event_id = ?",
    )
    .bind(event_id.to_string())
    .fetch_one(app.state.db.pool())
    .await
    .expect("persisted resource usage event");
    assert_eq!(
        persisted.get::<String, _>("resource_id"),
        resource_id.to_string()
    );
    assert_eq!(persisted.get::<String, _>("resource_version"), "0.1.0");
    assert_eq!(persisted.get::<String, _>("user_id"), admin.id.to_string());
    assert_eq!(
        persisted.get::<Option<String>, _>("session_id").as_deref(),
        Some("opaque-session-proof")
    );
    assert_eq!(persisted.get::<String, _>("outcome"), "success");
    assert_eq!(persisted.get::<i64, _>("duration_ms"), 1450);
    assert_eq!(persisted.get::<i64, _>("tokens_in"), 123);
    assert_eq!(persisted.get::<i64, _>("tokens_out"), 45);
}

async fn mark_setup_completed(app: &TestApp) {
    sqlx::query("UPDATE instance SET setup_completed = 1")
        .execute(app.state.db.pool())
        .await
        .expect("mark test project configured");
}

fn assert_safe_secret_metadata(secret: &Value) {
    assert!(secret["id"].as_str().is_some());
    assert!(secret["name"].as_str().is_some());
    assert!(secret["prefix"].as_str().is_some());
    assert!(secret.get("token").is_none());
    assert!(secret.get("token_hash").is_none());
}
