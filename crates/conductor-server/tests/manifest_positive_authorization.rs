//! Exhaustive positive HTTP proof for the protected route manifest.
//!
//! The manifest supplies the action, method, path, authentication class and
//! eligible role/scope. This file supplies only the domain fixtures needed to
//! drive the production Axum router far enough to observe a real allow
//! decision. It deliberately does not construct a proof-only router.

mod support;

use std::collections::BTreeSet;
use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode};
use conductor_auth::{hash_password_async, hash_token};
use conductor_domain::{
    role_has_permission, AuthenticationKind, AuthorizationAction as Action, ClientPlatform,
    CreateResourceRequest, CreateSubRoleRequest, CreateTagRequest, DraftFile, PermissionKey,
    PrimaryRole, RegisterClientRequest, ReleaseChannel, ReleaseResourceRequest, ResourceKind,
    ResourceVisibility, SecretScope, SetupRequest, TelemetryEventRequest, TelemetryEventStatus,
    TelemetryEventType, User, VersionMode,
};
use conductor_server::core::authorization::{
    AuthorizationDecisionObserver, AuthorizationEvent, AuthorizationResult, AuthorizationService,
    AuthorizationStage,
};
use conductor_server::core::resource_authoring::{
    resource_archive_media_type, resource_storage_payload, ResourceStorageArtifact,
};
use conductor_server::http::authorization::{
    route_manifest, RouteAuthentication, RouteSpec, RouteTargetSelector,
};
use conductor_storage::repos::{DraftContent, ReleaseContent};
use http_body_util::BodyExt;
use serde::Deserialize;
use serde_json::{json, Value};
use support::{test_app_with_authorization, TestApp};
use tower::ServiceExt;
use uuid::Uuid;

const EXPECTED_CONNECTION_ROLE_CASES: usize = 33;
const REVIEWED_ROUTE_INVENTORY: &str =
    include_str!("../../../docs/generated/req-004-route-inventory.json");

#[derive(Debug, Deserialize)]
struct ReviewedInventory {
    routes: Vec<ReviewedRoute>,
}

#[derive(Debug, Deserialize)]
struct ReviewedRoute {
    route_id: String,
    authentication: ReviewedAuthentication,
    role_baselines: ReviewedRoleBaselines,
}

#[derive(Debug, Deserialize)]
struct ReviewedAuthentication {
    #[serde(rename = "class")]
    class_name: String,
}

#[derive(Debug, Deserialize)]
struct ReviewedRoleBaselines {
    admin: ReviewedRoleBaseline,
    contribute: ReviewedRoleBaseline,
    user: ReviewedRoleBaseline,
}

impl ReviewedRoleBaselines {
    fn for_role(&self, role: PrimaryRole) -> &ReviewedRoleBaseline {
        match role {
            PrimaryRole::Admin => &self.admin,
            PrimaryRole::Contribute => &self.contribute,
            PrimaryRole::User => &self.user,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ReviewedRoleBaseline {
    outcome: ReviewedOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReviewedOutcome {
    Allow,
    Conditional,
    Deny,
}

#[derive(Default)]
struct RecordingObserver(Mutex<Vec<AuthorizationEvent>>);

impl RecordingObserver {
    fn clear(&self) {
        self.0.lock().expect("observer lock").clear();
    }

    fn events(&self) -> Vec<AuthorizationEvent> {
        self.0.lock().expect("observer lock").clone()
    }
}

impl AuthorizationDecisionObserver for RecordingObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        self.0.lock().expect("observer lock").push(event.clone());
    }
}

struct World {
    app: TestApp,
    observer: Arc<RecordingObserver>,
    project_id: Uuid,
    actor: User,
    browser_token: String,
}

impl World {
    async fn new(role: PrimaryRole) -> Self {
        let observer = Arc::new(RecordingObserver::default());
        let app = test_app_with_authorization(AuthorizationService::new(observer.clone())).await;
        let (instance, bootstrap_admin) = app
            .state
            .db
            .instance()
            .complete_setup(
                &SetupRequest {
                    project_name: format!("manifest-proof-{}", Uuid::new_v4().simple()),
                    display_name: Some("Manifest proof".into()),
                    bind_host: "127.0.0.1".into(),
                    bind_port: 4700,
                    public_url: None,
                    admin_email: format!("manifest-admin-{}@example.test", Uuid::new_v4().simple()),
                    admin_display_name: "Manifest Admin".into(),
                    admin_password: "unused".into(),
                    sso: None,
                },
                "unused-test-password-hash",
                "unused-test-jwt-secret",
                None,
            )
            .await
            .expect("configure manifest proof project");
        let project_id = instance.id;
        let actor = if role == PrimaryRole::Admin {
            bootstrap_admin
        } else {
            app.seed_user(role).await
        };
        let browser_token = app.token_for(&actor).await;
        Self {
            app,
            observer,
            project_id,
            actor,
            browser_token,
        }
    }

    async fn seed_member(&self, status: Option<&str>) -> User {
        let member = self.app.seed_user(PrimaryRole::User).await;
        if let Some(status) = status {
            sqlx::query("UPDATE users SET status = ? WHERE id = ?")
                .bind(status)
                .bind(member.id.to_string())
                .execute(self.app.state.db.pool())
                .await
                .expect("set fixture member status");
        }
        member
    }

    async fn seed_secret(&self, owner_id: Uuid, scopes: &[SecretScope]) -> Uuid {
        let raw = format!("evc_positive_secret_{}", Uuid::new_v4().simple());
        self.app
            .state
            .db
            .secrets()
            .insert(
                owner_id,
                "Positive manifest proof",
                "evc_posi",
                &hash_token(&raw),
                scopes,
                None,
            )
            .await
            .expect("seed secret")
            .id
    }

    async fn seed_connection_credential(&self, scope: SecretScope) -> (String, Uuid) {
        let raw = format!(
            "evc_positive_{}_{}",
            scope.as_str(),
            Uuid::new_v4().simple()
        );
        let credential = self
            .app
            .state
            .db
            .secrets()
            .insert(
                self.actor.id,
                "Positive manifest connection proof",
                "evc_posi",
                &hash_token(&raw),
                &[scope],
                None,
            )
            .await
            .expect("seed connection credential");
        (raw, credential.id)
    }

    async fn seed_installation(&self) -> Uuid {
        let request = RegisterClientRequest {
            installation_key: Uuid::new_v4(),
            display_name: "Manifest proof EvoFlux".into(),
            platform: ClientPlatform::Linux,
            evoflux_version: "1.0.0".into(),
            workspace_association: Some("manifest-proof".into()),
        };
        self.app
            .state
            .db
            .client_installations()
            .register(
                self.project_id,
                self.actor.id,
                Uuid::new_v4(),
                &hash_token(&format!("installation-{}", Uuid::new_v4())),
                &request,
            )
            .await
            .expect("seed client installation")
            .id
    }

    async fn seed_resource(&self, released: bool) -> SeededResource {
        let slug = format!("manifest-proof-{}", Uuid::new_v4().simple());
        let files = vec![DraftFile {
            path: "SKILL.md".into(),
            content: format!(
                "---\nname: {slug}\ndescription: Manifest authorization proof fixture.\n---\n\n# Proof\n"
            ),
        }, DraftFile {
            path: "notes.txt".into(),
            content: "manifest proof notes".into(),
        }];
        let artifact = self
            .app
            .state
            .artifacts
            .put_bundle(&files)
            .await
            .expect("store resource fixture bundle");
        let metadata_payload = resource_storage_payload(
            ResourceKind::Skill,
            &slug,
            "0.1.0",
            ResourceStorageArtifact {
                key: &artifact.key,
                sha256: &artifact.sha256,
                size: artifact.size,
                media_type: resource_archive_media_type(ResourceKind::Skill),
            },
            &files,
        );
        let draft = DraftContent {
            artifact_key: artifact.key.clone(),
            sha256: artifact.sha256.clone(),
            size: artifact.size,
            metadata_payload: metadata_payload.clone(),
        };
        let resource = self
            .app
            .state
            .db
            .resources()
            .create(
                self.project_id,
                &CreateResourceRequest {
                    kind: ResourceKind::Skill,
                    slug: slug.clone(),
                    name: "Manifest proof skill".into(),
                    description: None,
                    version: "0.1.0".into(),
                    visibility: ResourceVisibility::Shared,
                    payload: metadata_payload,
                    changelog: None,
                },
                self.actor.id,
                &draft,
            )
            .await
            .expect("seed resource");
        let version_id = if released {
            Some(
                self.app
                    .state
                    .db
                    .resources()
                    .release(
                        resource.id,
                        &ReleaseResourceRequest {
                            channel: ReleaseChannel::Published,
                            version_mode: VersionMode::Auto,
                            manual_version: None,
                            draft_revision: 0,
                            changelog: Some("Manifest proof release".into()),
                            beta_member_ids: vec![],
                            minimum_evoflux_version: None,
                        },
                        &ReleaseContent {
                            sha256: draft.sha256,
                            size: draft.size,
                            artifact_key: Some(draft.artifact_key),
                            updated_payload: Some(draft.metadata_payload.to_string()),
                        },
                        self.actor.id,
                    )
                    .await
                    .expect("release resource fixture")
                    .version_id,
            )
        } else {
            None
        };
        SeededResource {
            id: resource.id,
            version_id,
            slug,
        }
    }

    async fn seed_analytics_view(&self) -> Uuid {
        let (status, body) = self
            .app
            .post(
                "/api/analytics/views",
                Some(&self.browser_token),
                analytics_view_body(None),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "seed analytics view: {body}");
        Uuid::parse_str(body["id"].as_str().expect("analytics view id"))
            .expect("analytics view UUID")
    }

    async fn seed_known_actor_password(&self, password: &str) {
        let password_hash = hash_password_async(password.to_owned())
            .await
            .expect("hash manifest proof password");
        sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 0 WHERE id = ?")
            .bind(password_hash)
            .bind(self.actor.id.to_string())
            .execute(self.app.state.db.pool())
            .await
            .expect(
                "seed known manifest proof password without rotating the active fixture session",
            );
    }

    async fn seed_sub_role(&self) -> String {
        let suffix = Uuid::new_v4().simple().to_string();
        self.app
            .state
            .db
            .roles()
            .create_sub_role(&CreateSubRoleRequest {
                slug: format!("proof-{}", &suffix[..12]),
                name: "Manifest proof sub-role".into(),
                description: None,
                color: Some("#335577".into()),
            })
            .await
            .expect("seed manifest proof sub-role")
            .id
    }

    async fn seed_tag(&self) -> String {
        let suffix = Uuid::new_v4().simple().to_string();
        self.app
            .state
            .db
            .roles()
            .create_tag(&CreateTagRequest {
                slug: format!("proof-{}", &suffix[..12]),
                name: "Manifest proof tag".into(),
                description: None,
                color: Some("#557733".into()),
            })
            .await
            .expect("seed manifest proof tag")
            .id
    }

    async fn seed_activity_request(&self) -> String {
        let installation_id = self.seed_installation().await;
        let request_id = format!("manifest-{}", Uuid::new_v4().simple());
        let result = self
            .app
            .state
            .db
            .telemetry()
            .ingest(
                self.project_id,
                &self.actor,
                installation_id,
                "1.0.0",
                &[TelemetryEventRequest {
                    event_id: Uuid::new_v4(),
                    request_id: request_id.clone(),
                    session_id: Some("opaque-manifest-session".into()),
                    event_type: TelemetryEventType::Request,
                    sequence: 0,
                    agent_name: None,
                    provider: Some("manifest-provider".into()),
                    model: Some("manifest-model".into()),
                    response_model: None,
                    tokens_in: 1,
                    tokens_out: 1,
                    cache_read_tokens: 0,
                    reasoning_tokens: 0,
                    tool_use_tokens: 0,
                    duration_ms: 1,
                    tool_name: None,
                    tool_category: None,
                    status: TelemetryEventStatus::Success,
                    error_category: None,
                    estimated_cost_usd_micros: None,
                    cost_source: None,
                    evoflux_version: Some("1.0.0".into()),
                    resources: vec![],
                    reported_at: chrono::Utc::now(),
                }],
            )
            .await
            .expect("seed manifest activity request");
        assert_eq!(result.accepted, 1);
        request_id
    }

    async fn seed_deprecatable_resource(&self) -> SeededResource {
        let resource = self.seed_resource(true).await;
        let draft = self
            .app
            .state
            .db
            .resources()
            .draft_artifact(resource.id)
            .await
            .expect("load manifest proof draft")
            .expect("manifest proof draft exists");
        self.app
            .state
            .db
            .resources()
            .release(
                resource.id,
                &ReleaseResourceRequest {
                    channel: ReleaseChannel::Published,
                    version_mode: VersionMode::Auto,
                    manual_version: None,
                    draft_revision: 1,
                    changelog: Some("Manifest proof replacement release".into()),
                    beta_member_ids: vec![],
                    minimum_evoflux_version: None,
                },
                &ReleaseContent {
                    sha256: draft.sha256,
                    size: draft.size,
                    artifact_key: Some(draft.artifact_key),
                    updated_payload: Some(draft.metadata_payload.to_string()),
                },
                self.actor.id,
            )
            .await
            .expect("seed replacement release");
        resource
    }
}

struct SeededResource {
    id: Uuid,
    version_id: Option<Uuid>,
    slug: String,
}

enum RequestBody {
    Empty,
    Json(Value),
    Bytes {
        content_type: &'static str,
        value: Vec<u8>,
    },
}

struct PreparedRequest {
    path: String,
    headers: HeaderMap,
    body: RequestBody,
    expected_status: StatusCode,
    streaming: bool,
}

impl PreparedRequest {
    fn empty(route: &RouteSpec, expected_status: StatusCode) -> Self {
        Self::empty_at(route, &[], expected_status)
    }

    fn empty_at(
        route: &RouteSpec,
        replacements: &[(&str, String)],
        expected_status: StatusCode,
    ) -> Self {
        Self {
            path: api_path(route, replacements),
            headers: HeaderMap::new(),
            body: RequestBody::Empty,
            expected_status,
            streaming: false,
        }
    }

    fn json(
        route: &RouteSpec,
        replacements: &[(&str, String)],
        body: Value,
        expected_status: StatusCode,
    ) -> Self {
        Self {
            path: api_path(route, replacements),
            headers: HeaderMap::new(),
            body: RequestBody::Json(body),
            expected_status,
            streaming: false,
        }
    }

    fn bytes(
        route: &RouteSpec,
        replacements: &[(&str, String)],
        content_type: &'static str,
        value: Vec<u8>,
        expected_status: StatusCode,
    ) -> Self {
        Self {
            path: api_path(route, replacements),
            headers: HeaderMap::new(),
            body: RequestBody::Bytes {
                content_type,
                value,
            },
            expected_status,
            streaming: false,
        }
    }

    fn with_query(mut self, query: &str) -> Self {
        self.path.push('?');
        self.path.push_str(query);
        self
    }
}

fn api_path(route: &RouteSpec, replacements: &[(&str, String)]) -> String {
    let mut path = route.path.to_owned();
    for (parameter, value) in replacements {
        path = path.replace(parameter, value);
    }
    assert!(
        !path.contains('{') && !path.contains('}'),
        "unresolved fixture path for {}: {path}",
        route.route_id
    );
    format!("/api{path}")
}

fn analytics_view_body(revision: Option<u64>) -> Value {
    let mut body = json!({
        "name": format!("Manifest proof {}", Uuid::new_v4().simple()),
        "description": "Authorization proof",
        "visibility": "private",
        "definition": {
            "schema_version": 1,
            "preset": "executive",
            "density": "comfortable",
            "query": {
                "date_range": "last_30_days",
                "comparison": "previous_period"
            },
            "widgets": [{
                "id": "request-volume",
                "title": "Request volume",
                "visualization": "area",
                "metric": "requests",
                "group_by": "time",
                "size": "full",
                "limit": 10,
                "show_legend": false
            }]
        }
    });
    if let Some(revision) = revision {
        body["revision"] = json!(revision);
    }
    body
}

fn resource_create_body() -> Value {
    let slug = format!("http-proof-{}", Uuid::new_v4().simple());
    json!({
        "kind": "skill",
        "slug": slug,
        "name": "HTTP manifest proof",
        "description": null,
        "version": "0.1.0",
        "visibility": "shared",
        "payload": {
            "files": [{
                "path": "SKILL.md",
                "content": format!(
                    "---\nname: {slug}\ndescription: HTTP manifest proof fixture.\n---\n\n# Proof\n"
                )
            }]
        },
        "changelog": null
    })
}

fn zip_archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (path, content) in entries {
        writer.start_file(path, options).expect("start zip entry");
        writer
            .write_all(content.as_bytes())
            .expect("write zip entry");
    }
    writer.finish().expect("finish zip archive").into_inner()
}

fn skill_archive(slug: &str) -> Vec<u8> {
    let content = format!(
        "---\nname: {slug}\ndescription: Valid manifest authorization proof archive.\n---\n\n# Proof\n"
    );
    zip_archive(&[("SKILL.md", &content)])
}

fn plugin_archive() -> Vec<u8> {
    zip_archive(&[
        (
            "plugin.json",
            r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "manifest-proof-plugin",
  "version": "0.1.0",
  "description": "Valid manifest authorization proof plugin.",
  "extensions": {}
}"#,
        ),
        (
            "skills/manifest-proof/SKILL.md",
            "---\nname: manifest-proof\ndescription: Valid manifest proof skill.\n---\n\n# Proof\n",
        ),
    ])
}

async fn prepare_browser_request(world: &World, route: &RouteSpec) -> PreparedRequest {
    use Action::*;

    match route.action {
        SessionSelfRead
        | AuthorizationGrantsReadSelf
        | ProjectSsoRead
        | ProjectBrandingRead
        | ProjectSettingsRead
        | ProjectDashboardRead
        | MemberDirectoryList
        | MemberPendingCountRead
        | TaxonomySubRolesList
        | TaxonomyTagsList
        | ConnectionTokensSelfList => PreparedRequest::empty(route, StatusCode::OK),

        SessionPasswordChange => {
            world
                .seed_known_actor_password("manifest-current-password-123")
                .await;
            PreparedRequest::json(
                route,
                &[],
                json!({
                    "current_password": "manifest-current-password-123",
                    "new_password": "manifest-new-password-456"
                }),
                StatusCode::OK,
            )
        }
        ProjectSsoUpdate => PreparedRequest::json(
            route,
            &[],
            json!({
                "enabled": false,
                "provider": "oidc",
                "issuer_url": null,
                "client_id": null,
                "client_secret": null,
                "redirect_uri": null,
                "scopes": ["openid", "profile", "email"]
            }),
            StatusCode::OK,
        ),
        ProjectSettingsUpdate => PreparedRequest::json(
            route,
            &[],
            json!({
                "project_name": null,
                "display_name": "Updated manifest project",
                "description": "Successful authorization fixture",
                "public_url": null,
                "logo_url": null
            }),
            StatusCode::OK,
        ),
        ProjectNetworkUpdate => PreparedRequest::json(
            route,
            &[],
            json!({
                "bind_host": "127.0.0.1",
                "bind_port": 4811,
                "public_url": null,
                "realtime": {
                    "max_connections": 101,
                    "max_connections_per_secret": 7,
                    "heartbeat_seconds": 45
                }
            }),
            StatusCode::OK,
        ),
        ProjectDataPolicyUpdate => PreparedRequest::json(
            route,
            &[],
            json!({"collection_level": "L2"}),
            StatusCode::OK,
        ),
        ProjectStorageUpdate => PreparedRequest::json(
            route,
            &[],
            json!({
                "storage": serde_json::to_value(conductor_domain::StorageSettings::default())
                    .expect("serialize default storage settings"),
                "migrate_existing": true
            }),
            StatusCode::OK,
        ),
        MemberCreate => PreparedRequest::json(
            route,
            &[],
            json!({
                "email": format!("manifest-invite-{}@example.test", Uuid::new_v4().simple()),
                "display_name": "Manifest invited member",
                "primary_role": "user",
                "sub_role_ids": [],
                "tag_ids": []
            }),
            StatusCode::OK,
        ),
        TaxonomySubRoleCreate => {
            let suffix = Uuid::new_v4().simple().to_string();
            PreparedRequest::json(
                route,
                &[],
                json!({
                    "slug": format!("created-{}", &suffix[..12]),
                    "name": "Created manifest sub-role",
                    "description": "Successful authorization fixture",
                    "color": "#336699"
                }),
                StatusCode::OK,
            )
        }
        TaxonomyTagCreate => {
            let suffix = Uuid::new_v4().simple().to_string();
            PreparedRequest::json(
                route,
                &[],
                json!({
                    "slug": format!("created-{}", &suffix[..12]),
                    "name": "Created manifest tag",
                    "description": "Successful authorization fixture",
                    "color": "#669933"
                }),
                StatusCode::OK,
            )
        }
        ConnectionTokensSelfIssue => PreparedRequest::json(
            route,
            &[],
            json!({"name": "Manifest proof self token", "scopes": ["subscribe_resources"]}),
            StatusCode::OK,
        ),

        ProjectLogoUpload => PreparedRequest::bytes(
            route,
            &[],
            "image/png",
            vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            StatusCode::OK,
        ),
        ProjectLogoDelete => PreparedRequest::empty(route, StatusCode::OK),

        TaxonomySubRoleUpdate => {
            let sub_role_id = world.seed_sub_role().await;
            PreparedRequest::json(
                route,
                &[("{id}", sub_role_id)],
                json!({
                    "name": "Updated manifest sub-role",
                    "description": "Successful authorization fixture",
                    "color": "#224466"
                }),
                StatusCode::OK,
            )
        }
        TaxonomyTagUpdate => {
            let tag_id = world.seed_tag().await;
            PreparedRequest::json(
                route,
                &[("{id}", tag_id)],
                json!({
                    "name": "Updated manifest tag",
                    "description": "Successful authorization fixture",
                    "color": "#446622"
                }),
                StatusCode::OK,
            )
        }
        TaxonomySubRoleDelete => {
            let sub_role_id = world.seed_sub_role().await;
            PreparedRequest::empty_at(route, &[("{id}", sub_role_id)], StatusCode::OK)
        }
        TaxonomyTagDelete => {
            let tag_id = world.seed_tag().await;
            PreparedRequest::empty_at(route, &[("{id}", tag_id)], StatusCode::OK)
        }

        MemberPrivateRead | MemberInstallationsList | MemberConnectionTokensList => {
            PreparedRequest::empty_at(
                route,
                &[("{id}", world.actor.id.to_string())],
                StatusCode::OK,
            )
        }
        MemberConnectionTokenIssueSelf => PreparedRequest::json(
            route,
            &[("{id}", world.actor.id.to_string())],
            json!({"name": "Manifest proof", "scopes": ["subscribe_resources"]}),
            StatusCode::OK,
        ),
        MemberConnectionTokenRevoke => {
            let secret_id = world
                .seed_secret(world.actor.id, &[SecretScope::SubscribeResources])
                .await;
            PreparedRequest::json(
                route,
                &[
                    ("{id}", world.actor.id.to_string()),
                    ("{secret_id}", secret_id.to_string()),
                ],
                json!({}),
                StatusCode::OK,
            )
        }
        MemberAccessProfileUpdate => {
            let target = world.seed_member(None).await;
            PreparedRequest::json(
                route,
                &[("{id}", target.id.to_string())],
                json!({
                    "display_name": "Updated manifest target",
                    "primary_role": null,
                    "sub_role_ids": null,
                    "tag_ids": null
                }),
                StatusCode::OK,
            )
        }
        MemberApprove => {
            let target = world.seed_member(Some("pending")).await;
            PreparedRequest::json(
                route,
                &[("{id}", target.id.to_string())],
                json!({"primary_role": null, "sub_role_ids": null, "tag_ids": null}),
                StatusCode::OK,
            )
        }
        MemberDisable => {
            let target = world.seed_member(None).await;
            PreparedRequest::json(
                route,
                &[("{id}", target.id.to_string())],
                json!({}),
                StatusCode::OK,
            )
        }
        MemberEnable => {
            let target = world.seed_member(Some("disabled")).await;
            PreparedRequest::json(
                route,
                &[("{id}", target.id.to_string())],
                json!({}),
                StatusCode::OK,
            )
        }
        MemberPasswordReset => {
            let target = world.seed_member(None).await;
            PreparedRequest::json(
                route,
                &[("{id}", target.id.to_string())],
                json!({}),
                StatusCode::OK,
            )
        }

        MemberUsageSummaryRead | MemberActivityList | MemberToolsSummaryRead => {
            PreparedRequest::empty_at(
                route,
                &[("{id}", world.actor.id.to_string())],
                StatusCode::OK,
            )
        }
        MemberActivityDetailRead => {
            let request_id = world.seed_activity_request().await;
            PreparedRequest::empty_at(
                route,
                &[
                    ("{id}", world.actor.id.to_string()),
                    ("{request_id}", request_id),
                ],
                StatusCode::OK,
            )
        }
        AnalyticsResourceUsageRead => PreparedRequest::empty(route, StatusCode::OK),

        TaxonomyAssignmentRead | TaxonomyAssignmentSet => {
            let (entity_type, entity_id) = if world.actor.primary_role == PrimaryRole::Contribute {
                let resource = world.seed_resource(false).await;
                ("resource", resource.id)
            } else {
                let member = world.seed_member(None).await;
                ("member", member.id)
            };
            let replacements = [
                ("{entity_type}", entity_type.to_string()),
                ("{entity_id}", entity_id.to_string()),
            ];
            if route.action == TaxonomyAssignmentRead {
                PreparedRequest::empty_at(route, &replacements, StatusCode::OK)
            } else {
                PreparedRequest::json(route, &replacements, json!({"tag_ids": []}), StatusCode::OK)
            }
        }

        ConnectionTokensSelfRevoke => {
            let secret_id = world
                .seed_secret(world.actor.id, &[SecretScope::SubscribeResources])
                .await;
            PreparedRequest::json(
                route,
                &[("{id}", secret_id.to_string())],
                json!({}),
                StatusCode::OK,
            )
        }

        ResourcesList => PreparedRequest::empty(route, StatusCode::OK),
        ResourceCreate => PreparedRequest::json(route, &[], resource_create_body(), StatusCode::OK),
        ResourcePluginArchiveInspect => PreparedRequest::bytes(
            route,
            &[],
            "application/zip",
            plugin_archive(),
            StatusCode::OK,
        ),
        ResourcePluginArchiveImport => PreparedRequest::bytes(
            route,
            &[],
            "application/zip",
            plugin_archive(),
            StatusCode::OK,
        )
        .with_query("name=Manifest%20plugin&visibility=shared"),
        ResourceArchiveInspect => PreparedRequest::bytes(
            route,
            &[("{kind}", "skill".into())],
            "application/zip",
            skill_archive("manifest-inspect"),
            StatusCode::OK,
        ),
        ResourceArchiveImport => PreparedRequest::bytes(
            route,
            &[("{kind}", "skill".into())],
            "application/zip",
            skill_archive("manifest-import"),
            StatusCode::OK,
        )
        .with_query("slug=manifest-import&name=Manifest%20import&visibility=shared"),
        ResourceAuthoringGuideRead | ResourceAuthoringTemplateRead => {
            PreparedRequest::empty_at(route, &[("{kind}", "skill".into())], StatusCode::OK)
        }

        ResourceUpdate
        | ResourceArchive
        | ResourceDraftTreeRead
        | ResourceDraftFileSave
        | ResourceDraftEntryCreate
        | ResourceDraftEntryDelete
        | ResourceDraftEntryMove
        | ResourceDraftArchiveImport
        | ResourceDraftValidate
        | ResourceRelease
        | ResourceVersionsList
        | ResourceVersionDeprecate
        | ResourceVersionRestoreToDraft
        | ResourceAccessRead
        | ResourceAccessUpdate
        | ResourceMonitoringRead
        | ResourceInventoryMonitoringRead
        | ResourceFeedbackList => {
            let resource = match route.action {
                ResourceVersionDeprecate => world.seed_deprecatable_resource().await,
                ResourceVersionRestoreToDraft => world.seed_resource(true).await,
                _ => world.seed_resource(false).await,
            };
            let resource_id = resource.id.to_string();
            match route.action {
                ResourceUpdate => PreparedRequest::json(
                    route,
                    &[("{id}", resource_id)],
                    json!({"name": "Updated manifest resource", "description": null, "visibility": null}),
                    StatusCode::OK,
                ),
                ResourceArchive => PreparedRequest::json(
                    route,
                    &[("{id}", resource_id)],
                    json!({}),
                    StatusCode::OK,
                ),
                ResourceDraftTreeRead
                | ResourceDraftValidate
                | ResourceVersionsList
                | ResourceAccessRead
                | ResourceMonitoringRead
                | ResourceInventoryMonitoringRead
                | ResourceFeedbackList => {
                    PreparedRequest::empty_at(route, &[("{id}", resource_id)], StatusCode::OK)
                }
                ResourceDraftFileSave => PreparedRequest::json(
                    route,
                    &[("{id}", resource_id), ("{*path}", "notes.txt".into())],
                    json!({"content": "updated proof", "draft_revision": 0}),
                    StatusCode::OK,
                ),
                ResourceDraftEntryCreate => PreparedRequest::json(
                    route,
                    &[("{id}", resource_id)],
                    json!({"path": "created.txt", "content": "proof", "draft_revision": 0}),
                    StatusCode::OK,
                ),
                ResourceDraftEntryDelete => PreparedRequest::json(
                    route,
                    &[("{id}", resource_id)],
                    json!({"path": "notes.txt", "draft_revision": 0}),
                    StatusCode::OK,
                ),
                ResourceDraftEntryMove => PreparedRequest::json(
                    route,
                    &[("{id}", resource_id)],
                    json!({
                        "path": "notes.txt",
                        "destination_path": "moved.txt",
                        "draft_revision": 0
                    }),
                    StatusCode::OK,
                ),
                ResourceDraftArchiveImport => PreparedRequest::bytes(
                    route,
                    &[("{id}", resource_id)],
                    "application/zip",
                    skill_archive(&resource.slug),
                    StatusCode::OK,
                )
                .with_query("draft_revision=0"),
                ResourceRelease => PreparedRequest::json(
                    route,
                    &[("{id}", resource_id)],
                    json!({
                        "channel": "published",
                        "version_mode": "auto",
                        "manual_version": null,
                        "draft_revision": 0,
                        "changelog": null,
                        "beta_member_ids": [],
                        "minimum_evoflux_version": null
                    }),
                    StatusCode::OK,
                ),
                ResourceVersionDeprecate => PreparedRequest::json(
                    route,
                    &[
                        ("{id}", resource_id),
                        (
                            "{version_id}",
                            resource
                                .version_id
                                .expect("deprecatable version")
                                .to_string(),
                        ),
                    ],
                    json!({"reason": "manifest proof"}),
                    StatusCode::OK,
                ),
                ResourceVersionRestoreToDraft => PreparedRequest::json(
                    route,
                    &[
                        ("{id}", resource_id),
                        (
                            "{version_id}",
                            resource.version_id.expect("released version").to_string(),
                        ),
                    ],
                    json!({"draft_revision": 1, "confirm_deprecated": false}),
                    StatusCode::OK,
                ),
                ResourceAccessUpdate => PreparedRequest::json(
                    route,
                    &[("{id}", resource_id)],
                    json!({
                        "all_members": true,
                        "primary_roles": [],
                        "sub_role_ids": [],
                        "tag_ids": [],
                        "member_ids": []
                    }),
                    StatusCode::OK,
                ),
                HealthRead
                | SetupStatusRead
                | SetupComplete
                | AuthLogin
                | AuthSsoStart
                | AuthSsoCallback
                | ProjectLogoRead
                | SessionSelfRead
                | SessionPasswordChange
                | AuthorizationGrantsReadSelf
                | ProjectBrandingRead
                | ProjectDashboardRead
                | ProjectSettingsRead
                | ProjectSettingsUpdate
                | ProjectLogoUpload
                | ProjectLogoDelete
                | ProjectNetworkUpdate
                | ProjectSsoRead
                | ProjectSsoUpdate
                | ProjectStorageUpdate
                | ProjectDataPolicyUpdate
                | MemberDirectoryList
                | MemberPendingCountRead
                | MemberPrivateRead
                | MemberCreate
                | MemberApprove
                | MemberAccessProfileUpdate
                | MemberDisable
                | MemberEnable
                | MemberPasswordReset
                | MemberInstallationsList
                | MemberConnectionTokensList
                | MemberConnectionTokenIssueSelf
                | MemberConnectionTokenRevoke
                | MemberUsageSummaryRead
                | MemberActivityList
                | MemberActivityDetailRead
                | MemberToolsSummaryRead
                | TaxonomySubRolesList
                | TaxonomySubRoleCreate
                | TaxonomySubRoleUpdate
                | TaxonomySubRoleDelete
                | TaxonomyTagsList
                | TaxonomyTagCreate
                | TaxonomyTagUpdate
                | TaxonomyTagDelete
                | TaxonomyAssignmentRead
                | TaxonomyAssignmentSet
                | ConnectionTokensSelfList
                | ConnectionTokensSelfIssue
                | ConnectionTokensSelfRevoke
                | ResourcesList
                | ResourceCreate
                | ResourcePluginArchiveInspect
                | ResourcePluginArchiveImport
                | ResourceArchiveInspect
                | ResourceArchiveImport
                | ResourceAuthoringGuideRead
                | ResourceAuthoringTemplateRead
                | ResourceFeedbackSubmit
                | AnalyticsResourceUsageRead
                | AnalyticsViewsList
                | AnalyticsViewRead
                | AnalyticsViewCreate
                | AnalyticsViewUpdate
                | AnalyticsViewDelete
                | ClientRegister
                | ClientHeartbeat
                | ClientResourcesSnapshot
                | ClientResourcesChanges
                | ClientResourcesFetch
                | ClientResourceVersionRead
                | ClientResourceArtifactRead
                | ClientInventorySync
                | ClientTelemetryIngest
                | ClientResourceUsageIngest
                | ClientRealtimeEvents => unreachable!("outer resource action match"),
            }
        }

        ResourceFeedbackSubmit => {
            let resource = world.seed_resource(true).await;
            PreparedRequest::json(
                route,
                &[("{id}", resource.id.to_string())],
                json!({"rating": 5, "comment": "manifest proof"}),
                StatusCode::OK,
            )
        }

        AnalyticsViewsList => PreparedRequest::empty(route, StatusCode::OK),
        AnalyticsViewCreate => {
            PreparedRequest::json(route, &[], analytics_view_body(None), StatusCode::OK)
        }
        AnalyticsViewRead | AnalyticsViewUpdate | AnalyticsViewDelete => {
            let view_id = world.seed_analytics_view().await;
            match route.action {
                AnalyticsViewRead => PreparedRequest::empty_at(
                    route,
                    &[("{id}", view_id.to_string())],
                    StatusCode::OK,
                ),
                AnalyticsViewUpdate => PreparedRequest::json(
                    route,
                    &[("{id}", view_id.to_string())],
                    analytics_view_body(Some(1)),
                    StatusCode::OK,
                ),
                AnalyticsViewDelete => PreparedRequest::empty_at(
                    route,
                    &[("{id}", view_id.to_string())],
                    StatusCode::OK,
                )
                .with_query("revision=1"),
                HealthRead
                | SetupStatusRead
                | SetupComplete
                | AuthLogin
                | AuthSsoStart
                | AuthSsoCallback
                | SessionSelfRead
                | SessionPasswordChange
                | AuthorizationGrantsReadSelf
                | ProjectBrandingRead
                | ProjectDashboardRead
                | ProjectSettingsRead
                | ProjectSettingsUpdate
                | ProjectLogoRead
                | ProjectLogoUpload
                | ProjectLogoDelete
                | ProjectNetworkUpdate
                | ProjectSsoRead
                | ProjectSsoUpdate
                | ProjectStorageUpdate
                | ProjectDataPolicyUpdate
                | MemberDirectoryList
                | MemberPendingCountRead
                | MemberPrivateRead
                | MemberCreate
                | MemberApprove
                | MemberAccessProfileUpdate
                | MemberDisable
                | MemberEnable
                | MemberPasswordReset
                | MemberInstallationsList
                | MemberConnectionTokensList
                | MemberConnectionTokenIssueSelf
                | MemberConnectionTokenRevoke
                | MemberUsageSummaryRead
                | MemberActivityList
                | MemberActivityDetailRead
                | MemberToolsSummaryRead
                | TaxonomySubRolesList
                | TaxonomySubRoleCreate
                | TaxonomySubRoleUpdate
                | TaxonomySubRoleDelete
                | TaxonomyTagsList
                | TaxonomyTagCreate
                | TaxonomyTagUpdate
                | TaxonomyTagDelete
                | TaxonomyAssignmentRead
                | TaxonomyAssignmentSet
                | ConnectionTokensSelfList
                | ConnectionTokensSelfIssue
                | ConnectionTokensSelfRevoke
                | ResourcesList
                | ResourceCreate
                | ResourceUpdate
                | ResourceArchive
                | ResourceAccessRead
                | ResourceAccessUpdate
                | ResourceRelease
                | ResourceVersionsList
                | ResourceVersionDeprecate
                | ResourceVersionRestoreToDraft
                | ResourceMonitoringRead
                | ResourceInventoryMonitoringRead
                | ResourceFeedbackList
                | ResourceFeedbackSubmit
                | ResourceDraftTreeRead
                | ResourceDraftFileSave
                | ResourceDraftEntryCreate
                | ResourceDraftEntryDelete
                | ResourceDraftEntryMove
                | ResourceDraftValidate
                | ResourceDraftArchiveImport
                | ResourcePluginArchiveInspect
                | ResourcePluginArchiveImport
                | ResourceArchiveInspect
                | ResourceArchiveImport
                | ResourceAuthoringGuideRead
                | ResourceAuthoringTemplateRead
                | AnalyticsResourceUsageRead
                | AnalyticsViewsList
                | AnalyticsViewCreate
                | ClientRegister
                | ClientHeartbeat
                | ClientResourcesSnapshot
                | ClientResourcesChanges
                | ClientResourcesFetch
                | ClientResourceVersionRead
                | ClientResourceArtifactRead
                | ClientInventorySync
                | ClientTelemetryIngest
                | ClientResourceUsageIngest
                | ClientRealtimeEvents => unreachable!("outer analytics action match"),
            }
        }

        HealthRead
        | SetupStatusRead
        | SetupComplete
        | AuthLogin
        | AuthSsoStart
        | AuthSsoCallback
        | ProjectLogoRead
        | ClientRegister
        | ClientHeartbeat
        | ClientResourcesSnapshot
        | ClientResourcesChanges
        | ClientResourcesFetch
        | ClientResourceVersionRead
        | ClientResourceArtifactRead
        | ClientInventorySync
        | ClientTelemetryIngest
        | ClientResourceUsageIngest
        | ClientRealtimeEvents => unreachable!("non-browser action in browser fixture"),
    }
}

async fn prepare_connection_request(world: &World, route: &RouteSpec) -> PreparedRequest {
    use Action::*;

    match route.action {
        ClientResourcesSnapshot | ClientResourcesChanges => {
            PreparedRequest::empty(route, StatusCode::OK)
        }
        ClientResourcesFetch => {
            let installation_id = world.seed_installation().await;
            PreparedRequest::json(
                route,
                &[],
                json!({"installation_id": installation_id, "have_commit": null, "have": []}),
                StatusCode::OK,
            )
        }
        ClientResourceVersionRead | ClientResourceArtifactRead => {
            let resource = world.seed_resource(true).await;
            PreparedRequest::empty_at(
                route,
                &[
                    ("{id}", resource.id.to_string()),
                    (
                        "{version_id}",
                        resource.version_id.expect("released version").to_string(),
                    ),
                ],
                StatusCode::OK,
            )
        }
        ClientInventorySync => {
            let installation_id = world.seed_installation().await;
            PreparedRequest::json(
                route,
                &[],
                json!({"installation_id": installation_id, "items": []}),
                StatusCode::OK,
            )
        }
        ClientRegister => {
            let mut request = PreparedRequest::json(
                route,
                &[],
                json!({
                    "installation_key": Uuid::new_v4(),
                    "display_name": "Manifest proof EvoFlux",
                    "platform": "linux",
                    "evoflux_version": "1.0.0",
                    "workspace_association": "manifest-proof"
                }),
                StatusCode::OK,
            );
            request.headers.insert(
                "Idempotency-Key",
                HeaderValue::from_str(&Uuid::new_v4().to_string()).expect("idempotency header"),
            );
            request
        }
        ClientHeartbeat => {
            let installation_id = world.seed_installation().await;
            PreparedRequest::json(
                route,
                &[],
                json!({"installation_id": installation_id}),
                StatusCode::OK,
            )
        }
        ClientTelemetryIngest => {
            let installation_id = world.seed_installation().await;
            PreparedRequest::json(
                route,
                &[],
                json!({
                    "installation_id": installation_id,
                    "events": [{
                        "event_id": Uuid::new_v4(),
                        "request_id": format!("manifest-{}", Uuid::new_v4().simple()),
                        "session_id": null,
                        "event_type": "request",
                        "sequence": 0,
                        "agent_name": null,
                        "provider": null,
                        "model": null,
                        "response_model": null,
                        "tokens_in": 0,
                        "tokens_out": 0,
                        "cache_read_tokens": 0,
                        "reasoning_tokens": 0,
                        "tool_use_tokens": 0,
                        "duration_ms": 1,
                        "tool_name": null,
                        "tool_category": null,
                        "status": "success",
                        "error_category": null,
                        "estimated_cost_usd_micros": null,
                        "cost_source": null,
                        "evoflux_version": "1.0.0",
                        "resources": [],
                        "reported_at": chrono::Utc::now()
                    }]
                }),
                StatusCode::OK,
            )
        }
        ClientResourceUsageIngest => {
            let resource = world.seed_resource(true).await;
            PreparedRequest::json(
                route,
                &[],
                json!({
                    "events": [{
                        "event_id": Uuid::new_v4(),
                        "resource_id": resource.id,
                        "resource_version": "0.1.0",
                        "session_id": "opaque-manifest-session",
                        "outcome": "success",
                        "duration_ms": 1,
                        "tokens_in": 1,
                        "tokens_out": 1,
                        "occurred_at": chrono::Utc::now()
                    }]
                }),
                StatusCode::OK,
            )
        }
        ClientRealtimeEvents => {
            let mut request = PreparedRequest::empty(route, StatusCode::OK);
            request.streaming = true;
            request
        }

        HealthRead
        | SetupStatusRead
        | SetupComplete
        | AuthLogin
        | AuthSsoStart
        | AuthSsoCallback
        | ProjectLogoRead
        | SessionSelfRead
        | SessionPasswordChange
        | AuthorizationGrantsReadSelf
        | ProjectBrandingRead
        | ProjectDashboardRead
        | ProjectSettingsRead
        | ProjectSettingsUpdate
        | ProjectLogoUpload
        | ProjectLogoDelete
        | ProjectNetworkUpdate
        | ProjectSsoRead
        | ProjectSsoUpdate
        | ProjectStorageUpdate
        | ProjectDataPolicyUpdate
        | MemberDirectoryList
        | MemberPendingCountRead
        | MemberPrivateRead
        | MemberCreate
        | MemberApprove
        | MemberAccessProfileUpdate
        | MemberDisable
        | MemberEnable
        | MemberPasswordReset
        | MemberInstallationsList
        | MemberConnectionTokensList
        | MemberConnectionTokenIssueSelf
        | MemberConnectionTokenRevoke
        | MemberUsageSummaryRead
        | MemberActivityList
        | MemberActivityDetailRead
        | MemberToolsSummaryRead
        | TaxonomySubRolesList
        | TaxonomySubRoleCreate
        | TaxonomySubRoleUpdate
        | TaxonomySubRoleDelete
        | TaxonomyTagsList
        | TaxonomyTagCreate
        | TaxonomyTagUpdate
        | TaxonomyTagDelete
        | TaxonomyAssignmentRead
        | TaxonomyAssignmentSet
        | ConnectionTokensSelfList
        | ConnectionTokensSelfIssue
        | ConnectionTokensSelfRevoke
        | ResourcesList
        | ResourceCreate
        | ResourceUpdate
        | ResourceArchive
        | ResourceAccessRead
        | ResourceAccessUpdate
        | ResourceRelease
        | ResourceVersionsList
        | ResourceVersionDeprecate
        | ResourceVersionRestoreToDraft
        | ResourceMonitoringRead
        | ResourceInventoryMonitoringRead
        | ResourceFeedbackList
        | ResourceFeedbackSubmit
        | ResourceDraftTreeRead
        | ResourceDraftFileSave
        | ResourceDraftEntryCreate
        | ResourceDraftEntryDelete
        | ResourceDraftEntryMove
        | ResourceDraftValidate
        | ResourceDraftArchiveImport
        | ResourcePluginArchiveInspect
        | ResourcePluginArchiveImport
        | ResourceArchiveInspect
        | ResourceArchiveImport
        | ResourceAuthoringGuideRead
        | ResourceAuthoringTemplateRead
        | AnalyticsResourceUsageRead
        | AnalyticsViewsList
        | AnalyticsViewRead
        | AnalyticsViewCreate
        | AnalyticsViewUpdate
        | AnalyticsViewDelete => unreachable!("non-connection action in connection fixture"),
    }
}

async fn send_request(
    world: &World,
    route: &RouteSpec,
    credential: &str,
    prepared: PreparedRequest,
) -> (StatusCode, Value) {
    let method = Method::from_bytes(route.method.as_str().as_bytes()).expect("manifest method");
    let mut builder = Request::builder()
        .method(method)
        .uri(&prepared.path)
        .header(header::AUTHORIZATION, format!("Bearer {credential}"));
    for (name, value) in prepared.headers {
        if let Some(name) = name {
            builder = builder.header(name, value);
        }
    }
    let body = match prepared.body {
        RequestBody::Empty => Body::empty(),
        RequestBody::Json(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        RequestBody::Bytes {
            content_type,
            value,
        } => {
            builder = builder.header(header::CONTENT_TYPE, content_type);
            Body::from(value)
        }
    };
    let response = world
        .app
        .router
        .clone()
        .oneshot(builder.body(body).expect("build manifest request"))
        .await
        .expect("manifest response");
    let status = response.status();
    if prepared.streaming {
        return (status, Value::Null);
    }
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect manifest response")
        .to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, body)
}

async fn assert_success_response(world: &World, route: &RouteSpec, body: &Value, case: &str) {
    use Action::*;

    assert!(
        body.get("error").is_none() && body.get("error_code").is_none(),
        "successful fixture returned an error payload for {case}: {body}"
    );
    match route.action {
        SessionPasswordChange => {
            assert_eq!(body["user"]["id"], world.actor.id.to_string(), "{case}");
            assert!(
                body["token"]
                    .as_str()
                    .is_some_and(|token| !token.is_empty()),
                "{case}: {body}"
            );
        }
        ProjectSsoUpdate => {
            assert_eq!(body["enabled"], false, "{case}");
            assert_eq!(body["provider"], "oidc", "{case}");
        }
        ProjectSettingsUpdate => {
            assert_eq!(body["display_name"], "Updated manifest project", "{case}");
            assert_eq!(
                body["description"], "Successful authorization fixture",
                "{case}"
            );
        }
        ProjectNetworkUpdate => {
            assert_eq!(body["bind_port"], 4811, "{case}");
            assert_eq!(body["realtime"]["heartbeat_seconds"], 45, "{case}");
        }
        ProjectDataPolicyUpdate => {
            assert_eq!(body["data_policy"]["collection_level"], "L2", "{case}");
        }
        ProjectStorageUpdate => {
            assert_eq!(body["storage"]["backend"], "local", "{case}");
            assert_eq!(body["objects_copied"], 0, "{case}");
        }
        ProjectLogoUpload => {
            assert!(
                body["logo_url"]
                    .as_str()
                    .is_some_and(|url| url.starts_with("/api/project/logo?v=")),
                "{case}: {body}"
            );
            assert!(
                world
                    .app
                    .state
                    .db
                    .instance()
                    .logo_artifact()
                    .await
                    .expect("load uploaded logo metadata")
                    .is_some(),
                "{case}"
            );
        }
        ProjectLogoDelete => {
            assert_eq!(body["logo_url"], Value::Null, "{case}");
            assert!(
                world
                    .app
                    .state
                    .db
                    .instance()
                    .logo_artifact()
                    .await
                    .expect("load deleted logo metadata")
                    .is_none(),
                "{case}"
            );
        }
        MemberCreate => {
            assert_eq!(body["user"]["status"], "invited", "{case}");
            assert!(
                body["temporary_password"]
                    .as_str()
                    .is_some_and(|password| !password.is_empty()),
                "{case}: {body}"
            );
            let member_id = Uuid::parse_str(
                body["user"]["id"]
                    .as_str()
                    .expect("created member response ID"),
            )
            .expect("created member UUID");
            assert!(
                world
                    .app
                    .state
                    .db
                    .users()
                    .find_by_id(member_id)
                    .await
                    .expect("load created member")
                    .is_some(),
                "{case}"
            );
        }
        TaxonomySubRoleCreate => {
            assert_eq!(body["name"], "Created manifest sub-role", "{case}");
            assert!(body["id"].as_str().is_some(), "{case}: {body}");
        }
        TaxonomySubRoleUpdate => {
            assert_eq!(body["name"], "Updated manifest sub-role", "{case}");
            assert_eq!(body["color"], "#224466", "{case}");
        }
        TaxonomySubRoleDelete | TaxonomyTagDelete => {
            assert_eq!(body["deleted"], true, "{case}");
        }
        TaxonomyTagCreate => {
            assert_eq!(body["name"], "Created manifest tag", "{case}");
            assert!(body["id"].as_str().is_some(), "{case}: {body}");
        }
        TaxonomyTagUpdate => {
            assert_eq!(body["name"], "Updated manifest tag", "{case}");
            assert_eq!(body["color"], "#446622", "{case}");
        }
        ConnectionTokensSelfIssue | MemberConnectionTokenIssueSelf => {
            assert!(
                body["token"]
                    .as_str()
                    .is_some_and(|token| !token.is_empty()),
                "{case}: {body}"
            );
            assert_eq!(
                body["secret"]["owner_user_id"],
                world.actor.id.to_string(),
                "{case}"
            );
        }
        MemberActivityDetailRead => {
            assert!(
                body["request"]["request_id"]
                    .as_str()
                    .is_some_and(|request_id| request_id.starts_with("manifest-")),
                "{case}: {body}"
            );
            assert_eq!(body["request"]["total_tokens"], 2, "{case}");
            assert_eq!(body["events"].as_array().map(Vec::len), Some(1), "{case}");
        }
        ResourcePluginArchiveInspect => {
            assert_eq!(body["manifest"]["name"], "manifest-proof-plugin", "{case}");
            assert_eq!(body["validation"]["valid"], true, "{case}");
        }
        ResourcePluginArchiveImport => {
            assert_eq!(body["resource"]["kind"], "plugin", "{case}");
            assert_eq!(body["resource"]["status"], "draft", "{case}");
            assert_eq!(body["validation"]["valid"], true, "{case}");
        }
        ResourceArchiveInspect => {
            assert_eq!(body["kind"], "skill", "{case}");
            assert_eq!(body["validation"]["valid"], true, "{case}");
        }
        ResourceArchiveImport => {
            assert_eq!(body["resource"]["kind"], "skill", "{case}");
            assert_eq!(body["resource"]["status"], "draft", "{case}");
            assert_eq!(body["validation"]["valid"], true, "{case}");
        }
        ResourceDraftFileSave => {
            assert_eq!(body["revision"], 1, "{case}");
            assert!(
                body["files"].as_array().is_some_and(|files| files
                    .iter()
                    .any(|file| file["path"] == "notes.txt" && file["content"] == "updated proof")),
                "{case}: {body}"
            );
        }
        ResourceDraftEntryCreate => {
            assert_eq!(body["revision"], 1, "{case}");
            assert!(
                body["files"]
                    .as_array()
                    .is_some_and(|files| files.iter().any(|file| file["path"] == "created.txt")),
                "{case}: {body}"
            );
        }
        ResourceDraftEntryDelete => {
            assert_eq!(body["revision"], 1, "{case}");
            assert!(
                body["files"]
                    .as_array()
                    .is_some_and(|files| files.iter().all(|file| file["path"] != "notes.txt")),
                "{case}: {body}"
            );
        }
        ResourceDraftEntryMove => {
            assert_eq!(body["revision"], 1, "{case}");
            assert!(
                body["files"]
                    .as_array()
                    .is_some_and(|files| files.iter().any(|file| file["path"] == "moved.txt")),
                "{case}: {body}"
            );
        }
        ResourceDraftArchiveImport => {
            assert_eq!(body["tree"]["revision"], 1, "{case}");
            assert_eq!(body["validation"]["valid"], true, "{case}");
        }
        ResourceRelease => {
            assert_eq!(body["channel"], "published", "{case}");
            assert_eq!(body["version"], "0.1.0", "{case}");
            assert!(body["version_id"].as_str().is_some(), "{case}: {body}");
        }
        ResourceVersionDeprecate => {
            assert_eq!(body["status"], "deprecated", "{case}");
            assert_eq!(body["deprecation_reason"], "manifest proof", "{case}");
        }
        ResourceVersionRestoreToDraft => {
            assert_eq!(body["revision"], 2, "{case}");
            assert!(
                body["files"]
                    .as_array()
                    .is_some_and(|files| !files.is_empty()),
                "{case}: {body}"
            );
        }
        ClientResourceUsageIngest => {
            assert_eq!(body["accepted"], 1, "{case}");
            assert_eq!(body["duplicates"], 0, "{case}");
            assert_eq!(body["rejected"], 0, "{case}");
        }
        HealthRead
        | SetupStatusRead
        | SetupComplete
        | AuthLogin
        | AuthSsoStart
        | AuthSsoCallback
        | ProjectLogoRead
        | SessionSelfRead
        | AuthorizationGrantsReadSelf
        | ProjectSsoRead
        | ProjectBrandingRead
        | ProjectSettingsRead
        | ProjectDashboardRead
        | MemberDirectoryList
        | MemberPendingCountRead
        | MemberPrivateRead
        | MemberApprove
        | MemberAccessProfileUpdate
        | MemberDisable
        | MemberEnable
        | MemberPasswordReset
        | MemberInstallationsList
        | MemberConnectionTokensList
        | MemberConnectionTokenRevoke
        | MemberUsageSummaryRead
        | MemberActivityList
        | MemberToolsSummaryRead
        | TaxonomySubRolesList
        | TaxonomyTagsList
        | TaxonomyAssignmentRead
        | TaxonomyAssignmentSet
        | ConnectionTokensSelfList
        | ConnectionTokensSelfRevoke
        | ResourcesList
        | ResourceCreate
        | ResourceUpdate
        | ResourceArchive
        | ResourceAccessRead
        | ResourceAccessUpdate
        | ResourceVersionsList
        | ResourceMonitoringRead
        | ResourceInventoryMonitoringRead
        | ResourceFeedbackList
        | ResourceFeedbackSubmit
        | ResourceDraftTreeRead
        | ResourceDraftValidate
        | ResourceAuthoringGuideRead
        | ResourceAuthoringTemplateRead
        | AnalyticsResourceUsageRead
        | AnalyticsViewsList
        | AnalyticsViewRead
        | AnalyticsViewCreate
        | AnalyticsViewUpdate
        | AnalyticsViewDelete
        | ClientRegister
        | ClientHeartbeat
        | ClientResourcesSnapshot
        | ClientResourcesChanges
        | ClientResourcesFetch
        | ClientResourceVersionRead
        | ClientResourceArtifactRead
        | ClientInventorySync
        | ClientTelemetryIngest
        | ClientRealtimeEvents => {}
    }
}

fn route_requires_target(route: &RouteSpec) -> bool {
    match &route.authentication {
        RouteAuthentication::Browser(policy) => policy
            .alternatives
            .iter()
            .any(|alternative| !alternative.selector.can_resolve_at_route_boundary()),
        RouteAuthentication::Connection(policy) => !policy.selector.can_resolve_at_route_boundary(),
        RouteAuthentication::ExplicitPublic | RouteAuthentication::Bootstrap => false,
    }
}

type BrowserRoleCase = (String, String);

struct ReviewedBrowserMatrix {
    eligible: BTreeSet<BrowserRoleCase>,
    denied: BTreeSet<BrowserRoleCase>,
}

fn assert_reviewed_browser_role_matrix(routes: &[RouteSpec]) -> ReviewedBrowserMatrix {
    let inventory: ReviewedInventory = serde_json::from_str(REVIEWED_ROUTE_INVENTORY)
        .expect("parse checked-in REQ-004 route inventory");
    let reviewed_browser_routes = inventory
        .routes
        .iter()
        .filter(|route| route.authentication.class_name == "browser")
        .map(|route| route.route_id.clone())
        .collect::<BTreeSet<_>>();
    let production_browser_routes = routes
        .iter()
        .filter(|route| matches!(route.authentication, RouteAuthentication::Browser(_)))
        .map(|route| route.route_id.to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        production_browser_routes, reviewed_browser_routes,
        "browser route IDs drifted from the checked-in REQ-004 inventory"
    );

    let mut reviewed = ReviewedBrowserMatrix {
        eligible: BTreeSet::new(),
        denied: BTreeSet::new(),
    };
    for route in inventory
        .routes
        .iter()
        .filter(|route| route.authentication.class_name == "browser")
    {
        for role in PrimaryRole::ALL {
            let case = (route.route_id.clone(), role.as_str().to_owned());
            match route.role_baselines.for_role(role).outcome {
                ReviewedOutcome::Allow | ReviewedOutcome::Conditional => {
                    assert!(
                        reviewed.eligible.insert(case),
                        "duplicate reviewed role case"
                    );
                }
                ReviewedOutcome::Deny => {
                    assert!(reviewed.denied.insert(case), "duplicate reviewed role case");
                }
            }
        }
    }

    let mut production_eligible = BTreeSet::new();
    let mut production_denied = BTreeSet::new();
    for route in routes {
        let RouteAuthentication::Browser(policy) = &route.authentication else {
            continue;
        };
        for role in PrimaryRole::ALL {
            let case = (route.route_id.to_owned(), role.as_str().to_owned());
            if policy
                .alternatives
                .iter()
                .any(|alternative| role_has_permission(role, alternative.permission))
            {
                production_eligible.insert(case);
            } else {
                production_denied.insert(case);
            }
        }
    }
    assert_eq!(
        production_eligible, reviewed.eligible,
        "eligible browser role cases drifted from the checked-in REQ-004 inventory"
    );
    assert_eq!(
        production_denied, reviewed.denied,
        "denied browser role cases drifted from the checked-in REQ-004 inventory"
    );
    reviewed
}

fn assert_allowed_event(
    world: &World,
    route: &RouteSpec,
    expected_stage: AuthorizationStage,
    expected_credential_id: Option<Uuid>,
    case: &str,
) {
    let events = world.observer.events();
    let (authentication_kind, requirement_id, declared_permissions, required_scope) =
        match &route.authentication {
            RouteAuthentication::Browser(policy) => {
                assert!(
                    expected_credential_id.is_none(),
                    "browser case unexpectedly supplied a connection credential ID: {case}"
                );
                (
                    AuthenticationKind::BrowserSession,
                    policy.requirement_id,
                    policy
                        .alternatives
                        .iter()
                        .map(|alternative| alternative.permission)
                        .collect::<Vec<_>>(),
                    None,
                )
            }
            RouteAuthentication::Connection(policy) => (
                AuthenticationKind::ConnectionToken,
                policy.requirement_id,
                Vec::<PermissionKey>::new(),
                Some(policy.required_scope),
            ),
            RouteAuthentication::ExplicitPublic | RouteAuthentication::Bootstrap => {
                panic!("positive protected-route assertion used for {case}")
            }
        };
    if authentication_kind == AuthenticationKind::ConnectionToken {
        assert!(
            expected_credential_id.is_some(),
            "connection case has no safe credential ID: {case}"
        );
    }

    let route_actor_events = events
        .iter()
        .filter(|event| {
            event.normalized_route_id == route.route_id && event.actor_id == world.actor.id
        })
        .collect::<Vec<_>>();
    let matching_allowed = route_actor_events
        .iter()
        .copied()
        .filter(|event| {
            event.stage == expected_stage
                && event.authorization_result == AuthorizationResult::Allowed
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_allowed.len(),
        1,
        "expected exactly one {expected_stage:?} Allowed event for {case}; observed={}",
        serde_json::to_string(&events).expect("serialize observed events")
    );
    let request_id = matching_allowed[0].request_id;
    let request_events = route_actor_events
        .iter()
        .copied()
        .filter(|event| event.request_id == request_id)
        .collect::<Vec<_>>();
    assert_eq!(
        request_events.len(),
        route_actor_events.len(),
        "observer retained multiple requests after fixture reset for {case}"
    );
    assert!(
        !request_events
            .iter()
            .any(|event| event.authorization_result == AuthorizationResult::Denied),
        "observed a Denied decision in an allowed request for {case}: {}",
        serde_json::to_string(&request_events).expect("serialize request events")
    );

    for event in &request_events {
        assert_eq!(
            event.normalized_route_id, route.route_id,
            "route for {case}"
        );
        assert_eq!(event.method, route.method.as_str(), "method for {case}");
        assert_eq!(event.action, route.action, "action for {case}");
        assert_eq!(
            event.declared_requirement_id, requirement_id,
            "requirement for {case}"
        );
        assert_eq!(
            event.primary_role_snapshot, world.actor.primary_role,
            "role snapshot for {case}"
        );
        assert_eq!(
            event.authentication_kind, authentication_kind,
            "authentication kind for {case}"
        );
        assert_eq!(
            event.target_type, route.target_type,
            "target type for {case}"
        );
        assert_eq!(
            event.safe_credential_id, expected_credential_id,
            "safe credential ID for {case}"
        );
        assert_eq!(
            event.required_scope, required_scope,
            "required connection scope for {case}"
        );
        assert_eq!(
            event.evaluated_permissions, declared_permissions,
            "evaluated permissions for {case}"
        );
        match authentication_kind {
            AuthenticationKind::BrowserSession
                if event.authorization_result == AuthorizationResult::Allowed =>
            {
                let resolved = event
                    .resolved_permission
                    .expect("an allowed browser decision resolves a permission");
                assert!(
                    declared_permissions.contains(&resolved),
                    "resolved undeclared browser permission for {case}: {}",
                    resolved.as_str()
                );
            }
            AuthenticationKind::BrowserSession
            | AuthenticationKind::ConnectionToken
            | AuthenticationKind::Bootstrap
            | AuthenticationKind::Public => assert!(
                event.resolved_permission.is_none(),
                "non-browser-allow decision resolved a permission for {case}"
            ),
        }
    }

    let count = |stage, result| {
        request_events
            .iter()
            .filter(|event| event.stage == stage && event.authorization_result == result)
            .count()
    };
    let boundary_resolves = match &route.authentication {
        RouteAuthentication::Browser(policy) => policy.alternatives.iter().any(|alternative| {
            role_has_permission(world.actor.primary_role, alternative.permission)
                && alternative.selector.can_resolve_at_route_boundary()
        }),
        RouteAuthentication::Connection(policy) => policy.selector.can_resolve_at_route_boundary(),
        RouteAuthentication::ExplicitPublic | RouteAuthentication::Bootstrap => false,
    };
    match expected_stage {
        AuthorizationStage::RouteBoundary => {
            assert_eq!(
                count(
                    AuthorizationStage::RouteBoundary,
                    AuthorizationResult::Allowed
                ),
                1,
                "route-boundary Allowed flow for {case}"
            );
            assert_eq!(
                count(
                    AuthorizationStage::RouteBoundary,
                    AuthorizationResult::Preauthorized
                ),
                0,
                "unexpected route preauthorization for {case}"
            );
            let target_allowed = count(AuthorizationStage::Target, AuthorizationResult::Allowed);
            assert!(
                target_allowed <= 1,
                "route-resolved case emitted duplicate target decisions for {case}"
            );
            assert_eq!(
                request_events.len(),
                1 + target_allowed,
                "route-resolved case emitted an unexpected decision flow for {case}"
            );
        }
        AuthorizationStage::Target => {
            assert_eq!(
                count(AuthorizationStage::Target, AuthorizationResult::Allowed),
                1,
                "target Allowed flow for {case}"
            );
            if boundary_resolves {
                assert_eq!(
                    count(
                        AuthorizationStage::RouteBoundary,
                        AuthorizationResult::Allowed
                    ),
                    1,
                    "expected route-boundary Allowed before target evaluation for {case}"
                );
                assert_eq!(
                    count(
                        AuthorizationStage::RouteBoundary,
                        AuthorizationResult::Preauthorized
                    ),
                    0,
                    "unexpected preauthorization for route-resolved {case}"
                );
            } else {
                assert_eq!(
                    count(
                        AuthorizationStage::RouteBoundary,
                        AuthorizationResult::Allowed
                    ),
                    0,
                    "unexpected route-boundary Allowed for target-resolved {case}"
                );
                assert_eq!(
                    count(
                        AuthorizationStage::RouteBoundary,
                        AuthorizationResult::Preauthorized
                    ),
                    1,
                    "missing Preauthorized decision before target evaluation for {case}"
                );
            }
            assert_eq!(
                request_events.len(),
                2,
                "target-aware request must have one boundary and one target decision for {case}"
            );
        }
    }
}

#[tokio::test]
async fn every_eligible_browser_role_executes_the_production_manifest_route() {
    let manifest = route_manifest();
    let reviewed_matrix = assert_reviewed_browser_role_matrix(&manifest.routes);
    let mut cases = 0;

    for role in PrimaryRole::ALL {
        let mut world = World::new(role).await;
        for route in &manifest.routes {
            let RouteAuthentication::Browser(policy) = &route.authentication else {
                continue;
            };
            if !policy
                .alternatives
                .iter()
                .any(|alternative| role_has_permission(role, alternative.permission))
            {
                continue;
            }

            let prepared = prepare_browser_request(&world, route).await;
            let expected_status = prepared.expected_status;
            world.observer.clear();
            let browser_token = world.browser_token.clone();
            let (status, body) = send_request(&world, route, &browser_token, prepared).await;
            let case = format!("{} as {}", route.route_id, role.as_str());
            assert_eq!(status, expected_status, "status for {case}; body={body}");
            assert_success_response(&world, route, &body, &case).await;
            assert_allowed_event(
                &world,
                route,
                if route_requires_target(route) {
                    AuthorizationStage::Target
                } else {
                    AuthorizationStage::RouteBoundary
                },
                None,
                &case,
            );
            if route.action == Action::SessionPasswordChange {
                world.browser_token = body["token"]
                    .as_str()
                    .expect("password change returns a rotated browser token")
                    .to_owned();
            }
            cases += 1;
        }
    }

    assert_eq!(cases, reviewed_matrix.eligible.len());
}

#[tokio::test]
async fn every_connection_route_executes_with_the_correct_scope_for_each_current_role() {
    let manifest = route_manifest();
    let mut cases = 0;

    for role in PrimaryRole::ALL {
        let world = World::new(role).await;
        for route in &manifest.routes {
            let RouteAuthentication::Connection(policy) = &route.authentication else {
                continue;
            };

            let (credential, credential_id) = world
                .seed_connection_credential(policy.required_scope)
                .await;
            let prepared = prepare_connection_request(&world, route).await;
            let expected_status = prepared.expected_status;
            world.observer.clear();
            let (status, body) = send_request(&world, route, &credential, prepared).await;
            let case = format!(
                "{} as {} with {}",
                route.route_id,
                role.as_str(),
                policy.required_scope.as_str()
            );
            assert_eq!(status, expected_status, "status for {case}; body={body}");
            assert_success_response(&world, route, &body, &case).await;
            assert_allowed_event(
                &world,
                route,
                if route_requires_target(route) {
                    AuthorizationStage::Target
                } else {
                    AuthorizationStage::RouteBoundary
                },
                Some(credential_id),
                &case,
            );
            cases += 1;
        }
    }

    assert_eq!(cases, EXPECTED_CONNECTION_ROLE_CASES);
}

#[test]
fn target_requirement_classification_is_manifest_driven() {
    let manifest = route_manifest();
    let target_routes = manifest
        .routes
        .iter()
        .filter(|route| route_requires_target(route))
        .count();
    assert_eq!(target_routes, 56);
    assert!(manifest
        .routes
        .iter()
        .all(|route| match &route.authentication {
            RouteAuthentication::Browser(policy) => policy
                .alternatives
                .iter()
                .all(|alternative| { selector_is_typed(&alternative.selector) }),
            RouteAuthentication::Connection(policy) => selector_is_typed(&policy.selector),
            RouteAuthentication::ExplicitPublic | RouteAuthentication::Bootstrap => true,
        }));
}

fn selector_is_typed(selector: &RouteTargetSelector) -> bool {
    match selector {
        RouteTargetSelector::AllOf(items) | RouteTargetSelector::AnyOf(items) => {
            !items.is_empty() && items.iter().all(selector_is_typed)
        }
        _ => true,
    }
}
