mod support;

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{PrimaryRole, SecretScope, SetupRequest};
use serde_json::{json, Value};
use support::{test_app, TestApp};
use uuid::Uuid;

const RAW_TOKEN: &str = "evc_smart_fetch_test_secret";

async fn configured_app() -> (TestApp, String, Uuid) {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "smart-fetch-test".into(),
                display_name: Some("Smart fetch test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "fetch-admin@example.test".into(),
                admin_display_name: "Fetch Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let admin_token = app.token_for(&admin).await;
    let member = app.seed_user(PrimaryRole::User).await;
    app.state
        .db
        .secrets()
        .insert(
            member.id,
            "Smart fetch",
            "evc_fetc",
            &hash_token(RAW_TOKEN),
            &[SecretScope::SubscribeResources],
            None,
        )
        .await
        .expect("seed connection token");
    let (status, registration) = app
        .post_with_headers(
            "/api/v1/client/register",
            Some(RAW_TOKEN),
            idempotency_headers(),
            json!({
                "installation_key": Uuid::new_v4(),
                "display_name": "EvoFlux smart fetch",
                "platform": "linux",
                "evoflux_version": "1.0.0",
                "workspace_association": "smart-fetch"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{registration}");
    let installation_id = Uuid::parse_str(
        registration["installation"]["id"]
            .as_str()
            .expect("installation id"),
    )
    .unwrap();
    (app, admin_token, installation_id)
}

fn idempotency_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Idempotency-Key",
        HeaderValue::from_str(&Uuid::new_v4().to_string()).unwrap(),
    );
    headers
}

async fn create_released_skill(app: &TestApp, admin_token: &str) -> (Uuid, Uuid) {
    let (status, resource) = app
        .post(
            "/api/resources",
            Some(admin_token),
            json!({
                "kind": "skill",
                "slug": "incident-summary",
                "name": "Incident summary",
                "description": "Summarize incidents",
                "version": "0.1.0",
                "visibility": "shared",
                "payload": {
                    "files": [{
                        "path": "SKILL.md",
                        "content": "---\nname: incident-summary\ndescription: Summarize incidents with evidence.\n---\n\n# Incident summary\n"
                    }]
                },
                "changelog": null
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resource}");
    let resource_id = Uuid::parse_str(resource["id"].as_str().unwrap()).unwrap();
    let (status, release) = app
        .post(
            &format!("/api/resources/{resource_id}/release"),
            Some(admin_token),
            json!({
                "channel": "published",
                "version_mode": "auto",
                "manual_version": null,
                "draft_revision": 0,
                "changelog": "Initial release",
                "beta_member_ids": [],
                "minimum_evoflux_version": null
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{release}");
    let version_id = Uuid::parse_str(release["version_id"].as_str().unwrap()).unwrap();
    (resource_id, version_id)
}

async fn fetch(app: &TestApp, installation_id: Uuid, body: Value) -> Value {
    let (status, response) = app
        .post(
            "/api/v1/resources/fetch",
            Some(RAW_TOKEN),
            json!({
                "installation_id": installation_id,
                "have_commit": body.get("have_commit").cloned().unwrap_or(Value::Null),
                "have": body.get("have").cloned().unwrap_or_else(|| json!([]))
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    response
}

#[tokio::test]
async fn smart_fetch_negotiates_delta_objects_and_tombstones() {
    let (app, admin_token, installation_id) = configured_app().await;
    let (resource_id, version_id) = create_released_skill(&app, &admin_token).await;

    let initial = fetch(&app, installation_id, json!({})).await;
    assert_eq!(initial["schema_version"], 1);
    assert_eq!(initial["up_to_date"], false);
    assert_eq!(initial["entries"].as_array().map(Vec::len), Some(1));
    assert_eq!(initial["objects"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        initial["entries"][0]["resource_id"],
        resource_id.to_string()
    );
    assert_eq!(initial["entries"][0]["version_id"], version_id.to_string());
    assert_eq!(initial["entries"][0]["kind"], "skill");
    assert_eq!(initial["entries"][0]["bundle"]["schema_version"], 2);
    assert_eq!(initial["commit"]["id"].as_str().map(str::len), Some(64));
    assert_eq!(
        initial["commit"]["tree_sha256"].as_str().map(str::len),
        Some(64)
    );

    let commit = initial["commit"]["id"].as_str().unwrap();
    let up_to_date = fetch(
        &app,
        installation_id,
        json!({"have_commit": commit, "have": []}),
    )
    .await;
    assert_eq!(up_to_date["up_to_date"], true);
    assert!(up_to_date["entries"].as_array().unwrap().is_empty());
    assert!(up_to_date["objects"].as_array().unwrap().is_empty());

    let artifact_sha256 = initial["entries"][0]["bundle"]["artifact_sha256"]
        .as_str()
        .unwrap();
    let negotiated = fetch(
        &app,
        installation_id,
        json!({
            "have_commit": "0".repeat(64),
            "have": [{
                "resource_id": resource_id,
                "version_id": version_id,
                "artifact_sha256": artifact_sha256
            }]
        }),
    )
    .await;
    assert_eq!(negotiated["up_to_date"], false);
    assert!(negotiated["entries"].as_array().unwrap().is_empty());
    assert!(negotiated["objects"].as_array().unwrap().is_empty());

    let artifact_href = initial["objects"][0]["href"].as_str().unwrap();
    let (status, headers, bytes) = app
        .get_bytes_with_headers(artifact_href, Some(RAW_TOKEN), HeaderMap::new())
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!bytes.is_empty());
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "private, max-age=31536000, immutable"
    );
    let etag = headers[header::ETAG].clone();
    let mut conditional = HeaderMap::new();
    conditional.insert(header::IF_NONE_MATCH, etag);
    let (status, _, bytes) = app
        .get_bytes_with_headers(artifact_href, Some(RAW_TOKEN), conditional)
        .await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(bytes.is_empty());

    let (status, archived) = app
        .post(
            &format!("/api/resources/{resource_id}/archive"),
            Some(&admin_token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{archived}");
    let removed = fetch(
        &app,
        installation_id,
        json!({
            "have_commit": commit,
            "have": [{
                "resource_id": resource_id,
                "version_id": version_id,
                "artifact_sha256": artifact_sha256
            }]
        }),
    )
    .await;
    assert!(removed["entries"].as_array().unwrap().is_empty());
    assert_eq!(
        removed["tombstones"][0]["resource_id"],
        resource_id.to_string()
    );
    assert_ne!(removed["commit"]["id"], initial["commit"]["id"]);
}
