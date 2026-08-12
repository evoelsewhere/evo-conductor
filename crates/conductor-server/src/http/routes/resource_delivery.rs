use std::collections::{BTreeMap, HashMap, HashSet};
use std::str::FromStr;

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use conductor_domain::{
    ConductorError, CreateDraftFileRequest, CreateResourceRequest, DeleteDraftEntryRequest,
    DraftFile, DraftFileTree, EffectiveResourceVersion, ManagedResource, MoveDraftEntryRequest,
    PrimaryRole, ReleaseResourceRequest, ReleaseResourceResult, ResourceBundleKind, ResourceChange,
    ResourceChangePage, ResourceFetchCommit, ResourceFetchEntry, ResourceFetchObject,
    ResourceFetchRequest, ResourceFetchResponse, ResourceFetchTombstone, ResourceInventoryRequest,
    ResourceInventoryResponse, ResourceKind, ResourceTargetMode, ResourceValidation,
    ResourceVisibility, SaveDraftFileRequest, SecretScope, SemanticVersion, VersionMode,
};
use conductor_storage::repos::{
    DraftArtifact, DraftContent, DraftWriteError, ReleaseContent, ReleaseResourceError,
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::core::error::{ApiError, ApiResult};
use crate::core::resource_authoring::{
    archive_source_metadata, import_zip, resource_archive_media_type, resource_storage_payload,
    safe_relative_path, set_target_modes, starter_files, validate_draft, versioned_plugin_files,
    MAX_EDITABLE_FILE_BYTES,
};
use crate::core::state::AppState;
use crate::http::extractors::{authenticate_connection_secret, AuthUser};

const CHANGE_SCHEMA_VERSION: u8 = 2;
const FETCH_SCHEMA_VERSION: u8 = 1;
const DEFAULT_CHANGE_LIMIT: u32 = 100;
const MAX_CHANGE_LIMIT: u32 = 500;
const MAX_FETCH_HAVE: usize = 5_000;
const MAX_FETCH_STABILIZE_ATTEMPTS: usize = 4;
const PLUGIN_IMPORT_CHANGELOG: &str = "Imported plugin package";
const AGENT_IMPORT_CHANGELOG: &str = "Imported EvoFlux Agent package";
const SKILL_IMPORT_CHANGELOG: &str = "Imported EvoFlux Skill bundle";

#[derive(Debug, Serialize)]
pub struct ResourceGuide {
    kind: ResourceKind,
    schema_version: u8,
    title: &'static str,
    summary: &'static str,
    required_entries: Vec<&'static str>,
    max_files: usize,
    max_editable_file_bytes: usize,
}

pub async fn guide(
    AuthUser(actor): AuthUser,
    Path(kind): Path<String>,
) -> ApiResult<Json<ResourceGuide>> {
    require_author(actor.primary_role)?;
    let kind = parse_kind(&kind)?;
    let (title, summary, required_entries) = match kind {
        ResourceKind::Agent => (
            "EvoFlux Agent",
            "A Markdown Agent definition with YAML frontmatter and a system prompt.",
            vec!["<slug>.md"],
        ),
        ResourceKind::Skill => (
            "EvoFlux Skill",
            "A standalone Skill bundle whose root contains SKILL.md.",
            vec!["SKILL.md"],
        ),
        ResourceKind::Plugin => (
            "Portable Agent Plugin",
            "An Agent Plugins 1.0 package. Conductor publishes immutable archives; EvoFlux performs local trust review before enablement.",
            vec!["plugin.json"],
        ),
        ResourceKind::Workflow | ResourceKind::Command => (
            "Structured resource",
            "A versioned structured resource distributed by Conductor.",
            vec![],
        ),
    };
    Ok(Json(ResourceGuide {
        kind,
        schema_version: 1,
        title,
        summary,
        required_entries,
        max_files: crate::core::resource_authoring::MAX_DRAFT_FILES,
        max_editable_file_bytes: MAX_EDITABLE_FILE_BYTES,
    }))
}

pub async fn template(
    AuthUser(actor): AuthUser,
    Path(kind): Path<String>,
) -> ApiResult<Json<DraftFileTree>> {
    require_author(actor.primary_role)?;
    let kind = parse_kind(&kind)?;
    Ok(Json(DraftFileTree {
        resource_id: Uuid::nil(),
        revision: 0,
        files: starter_files(kind, "new-resource", "New resource"),
    }))
}

pub async fn draft_tree(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
) -> ApiResult<Json<DraftFileTree>> {
    authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    let tree = current_draft(&state, resource_id).await?;
    Ok(Json(tree))
}

pub async fn save_draft_file(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path((resource_id, path)): Path<(Uuid, String)>,
    Json(request): Json<SaveDraftFileRequest>,
) -> ApiResult<Json<DraftFileTree>> {
    authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    if !safe_relative_path(&path) {
        return Err(ConductorError::msg("unsafe draft path").into());
    }
    if request.content.len() > MAX_EDITABLE_FILE_BYTES {
        return Err(ConductorError::msg("editable files are limited to 1 MiB").into());
    }
    let mut tree = current_draft(&state, resource_id).await?;
    if tree.revision != request.draft_revision {
        return Err(ConductorError::Conflict("draft_revision_conflict".into()).into());
    }
    if let Some(file) = tree.files.iter_mut().find(|file| file.path == path) {
        file.content = request.content;
    } else {
        tree.files.push(DraftFile {
            path,
            content: request.content,
        });
    }
    replace_draft_files(&state, resource_id, tree.files, request.draft_revision).await
}

pub async fn create_draft_file(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Json(mut request): Json<CreateDraftFileRequest>,
) -> ApiResult<Json<DraftFileTree>> {
    authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    request.path = request.path.trim().trim_matches('/').to_string();
    validate_editable_path(&request.path)?;
    if request.content.len() > MAX_EDITABLE_FILE_BYTES {
        return Err(ConductorError::msg("editable files are limited to 1 MiB").into());
    }
    let tree = current_draft(&state, resource_id).await?;
    if tree.revision != request.draft_revision {
        return Err(ConductorError::Conflict("draft_revision_conflict".into()).into());
    }
    if tree.files.len() >= crate::core::resource_authoring::MAX_DRAFT_FILES {
        return Err(ConductorError::msg("draft file limit reached").into());
    }
    if tree
        .files
        .iter()
        .any(|file| paths_overlap(&file.path, &request.path))
    {
        return Err(ConductorError::Conflict("draft_path_already_exists".into()).into());
    }
    replace_draft_files(
        &state,
        resource_id,
        tree.files
            .into_iter()
            .chain(std::iter::once(DraftFile {
                path: request.path,
                content: request.content,
            }))
            .collect(),
        request.draft_revision,
    )
    .await
}

pub async fn move_draft_entry(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Json(mut request): Json<MoveDraftEntryRequest>,
) -> ApiResult<Json<DraftFileTree>> {
    let resource = authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    request.path = request.path.trim().trim_matches('/').to_string();
    request.destination_path = request
        .destination_path
        .trim()
        .trim_matches('/')
        .to_string();
    validate_editable_path(&request.path)?;
    validate_editable_path(&request.destination_path)?;
    if request.path == request.destination_path {
        return Err(ConductorError::msg("destination path must be different").into());
    }
    if request
        .destination_path
        .starts_with(&format!("{}/", request.path))
    {
        return Err(ConductorError::msg("an entry cannot be moved inside itself").into());
    }
    protect_required_entry(&resource, &request.path)?;
    let tree = current_draft(&state, resource_id).await?;
    if tree.revision != request.draft_revision {
        return Err(ConductorError::Conflict("draft_revision_conflict".into()).into());
    }
    let source_prefix = format!("{}/", request.path);
    let destination_prefix = format!("{}/", request.destination_path);
    let found = tree
        .files
        .iter()
        .any(|file| file.path == request.path || file.path.starts_with(&source_prefix));
    if !found {
        return Err(ConductorError::NotFound("draft entry".into()).into());
    }
    let files: Vec<_> = tree
        .files
        .into_iter()
        .map(|mut file| {
            if file.path == request.path {
                file.path.clone_from(&request.destination_path);
            } else if let Some(suffix) = file.path.strip_prefix(&source_prefix) {
                file.path = format!("{destination_prefix}{suffix}");
            }
            file
        })
        .collect();
    ensure_unique_draft_paths(&files)?;
    replace_draft_files(&state, resource_id, files, request.draft_revision).await
}

pub async fn delete_draft_entry(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Json(mut request): Json<DeleteDraftEntryRequest>,
) -> ApiResult<Json<DraftFileTree>> {
    let resource = authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    request.path = request.path.trim().trim_matches('/').to_string();
    validate_editable_path(&request.path)?;
    protect_required_entry(&resource, &request.path)?;
    let tree = current_draft(&state, resource_id).await?;
    if tree.revision != request.draft_revision {
        return Err(ConductorError::Conflict("draft_revision_conflict".into()).into());
    }
    let source_prefix = format!("{}/", request.path);
    let previous_count = tree.files.len();
    let files: Vec<_> = tree
        .files
        .into_iter()
        .filter(|file| file.path != request.path && !file.path.starts_with(&source_prefix))
        .collect();
    if files.len() == previous_count {
        return Err(ConductorError::NotFound("draft entry".into()).into());
    }
    replace_draft_files(&state, resource_id, files, request.draft_revision).await
}

async fn current_draft(state: &AppState, resource_id: Uuid) -> ApiResult<DraftFileTree> {
    let draft = state
        .db
        .resources()
        .draft_artifact(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource source".into()))?;
    hydrate_draft(state, draft).await
}

async fn replace_draft_files(
    state: &AppState,
    resource_id: Uuid,
    mut files: Vec<DraftFile>,
    draft_revision: u64,
) -> ApiResult<Json<DraftFileTree>> {
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let resource = state
        .db
        .resources()
        .find_by_id(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
    let artifact =
        state.artifacts.put_bundle(&files).await.map_err(|error| {
            ConductorError::msg(format!("object storage write failed: {error}"))
        })?;
    let metadata_payload = resource_storage_payload(
        resource.kind,
        &resource.slug,
        &resource.version,
        &artifact.key,
        &artifact.sha256,
        artifact.size,
        resource_archive_media_type(resource.kind),
        &files,
    );
    let draft = DraftContent {
        artifact_key: artifact.key,
        sha256: artifact.sha256,
        size: artifact.size,
        metadata_payload,
    };
    match state
        .db
        .resources()
        .replace_draft_artifact(resource_id, &draft, draft_revision)
        .await
    {
        Ok(stored) => Ok(Json(DraftFileTree {
            resource_id,
            revision: stored.revision,
            files,
        })),
        Err(DraftWriteError::NotFound) => Err(ConductorError::NotFound("resource".into()).into()),
        Err(DraftWriteError::Conflict) => {
            Err(ConductorError::Conflict("draft_revision_conflict".into()).into())
        }
        Err(DraftWriteError::Database(error)) => Err(ApiError::from(error)),
    }
}

pub(super) async fn hydrate_draft(
    state: &AppState,
    draft: DraftArtifact,
) -> ApiResult<DraftFileTree> {
    let bytes = state
        .artifacts
        .read(&draft.artifact_key)
        .await
        .map_err(|error| ConductorError::msg(format!("object storage read failed: {error}")))?;
    if hex::encode(Sha256::digest(&bytes)) != draft.sha256
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != draft.size
    {
        return Err(ConductorError::msg("draft artifact integrity check failed").into());
    }
    let files = tokio::task::spawn_blocking(move || import_zip(bytes))
        .await
        .map_err(|_| ConductorError::Internal)?
        .map_err(ConductorError::msg)?;
    Ok(DraftFileTree {
        resource_id: draft.resource_id,
        revision: draft.revision,
        files,
    })
}

fn validate_editable_path(path: &str) -> ApiResult<()> {
    if !safe_relative_path(path) {
        return Err(ConductorError::msg("unsafe draft path").into());
    }
    Ok(())
}

fn protect_required_entry(resource: &ManagedResource, path: &str) -> ApiResult<()> {
    let required = match resource.kind {
        ResourceKind::Plugin => "plugin.json".to_string(),
        ResourceKind::Skill => "SKILL.md".to_string(),
        ResourceKind::Agent => format!("{}.md", resource.slug),
        ResourceKind::Workflow | ResourceKind::Command => format!("{}.json", resource.slug),
    };
    if required == path || required.starts_with(&format!("{path}/")) {
        return Err(ConductorError::msg(format!(
            "{required} is required and cannot be moved or deleted"
        ))
        .into());
    }
    Ok(())
}

fn ensure_unique_draft_paths(files: &[DraftFile]) -> ApiResult<()> {
    let paths: Vec<_> = files.iter().map(|file| file.path.to_lowercase()).collect();
    let mut unique = HashSet::new();
    if paths.iter().any(|path| !unique.insert(path))
        || paths.iter().enumerate().any(|(index, path)| {
            paths
                .iter()
                .skip(index + 1)
                .any(|other| paths_overlap(path, other))
        })
    {
        return Err(ConductorError::Conflict("draft_path_already_exists".into()).into());
    }
    Ok(())
}

fn paths_overlap(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
        || left
            .to_lowercase()
            .starts_with(&format!("{}/", right.to_lowercase()))
        || right
            .to_lowercase()
            .starts_with(&format!("{}/", left.to_lowercase()))
}

#[derive(Debug, Deserialize)]
pub struct ImportQuery {
    draft_revision: u64,
}

#[derive(Debug, Serialize)]
pub struct DraftImportResponse {
    tree: DraftFileTree,
    validation: ResourceValidation,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginArchiveManifest {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PluginArchiveInspection {
    manifest: PluginArchiveManifest,
    validation: ResourceValidation,
    file_count: usize,
    total_uncompressed_bytes: u64,
    skill_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct CreatePluginArchiveQuery {
    name: String,
    visibility: ResourceVisibility,
}

#[derive(Debug, Serialize)]
pub struct PluginArchiveCreateResponse {
    resource: ManagedResource,
    validation: ResourceValidation,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceArchiveMetadata {
    slug: Option<String>,
    version: Option<String>,
    description: Option<String>,
    primary_source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResourceArchiveInspection {
    kind: ResourceKind,
    metadata: ResourceArchiveMetadata,
    validation: ResourceValidation,
    file_count: usize,
    total_uncompressed_bytes: u64,
}

#[derive(Debug, Deserialize)]
pub struct CreateResourceArchiveQuery {
    slug: String,
    name: String,
    visibility: ResourceVisibility,
    modes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResourceArchiveCreateResponse {
    resource: ManagedResource,
    validation: ResourceValidation,
}

pub async fn inspect_resource_archive(
    AuthUser(actor): AuthUser,
    Path(kind): Path<String>,
    body: Bytes,
) -> ApiResult<Json<ResourceArchiveInspection>> {
    require_author(actor.primary_role)?;
    let kind = parse_import_kind(&kind)?;
    let files = extract_resource_archive(body).await?;
    Ok(Json(inspect_resource_files(kind, &files, None)))
}

fn parse_archive_modes(value: Option<&str>) -> ApiResult<Vec<ResourceTargetMode>> {
    let Some(value) = value else {
        return Ok(ResourceTargetMode::ALL.to_vec());
    };
    let mut selected = Vec::new();
    for raw in value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let mode = ResourceTargetMode::parse(raw)
            .ok_or_else(|| ConductorError::msg("modes may contain only work, coding and aim"))?;
        if !selected.contains(&mode) {
            selected.push(mode);
        }
    }
    if selected.is_empty() {
        return Err(ConductorError::msg("select at least one resource mode").into());
    }
    Ok(ResourceTargetMode::ALL
        .into_iter()
        .filter(|mode| selected.contains(mode))
        .collect())
}

pub async fn create_resource_archive(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(kind): Path<String>,
    Query(query): Query<CreateResourceArchiveQuery>,
    body: Bytes,
) -> ApiResult<Json<ResourceArchiveCreateResponse>> {
    require_author(actor.primary_role)?;
    let kind = parse_import_kind(&kind)?;
    let mut files = extract_resource_archive(body).await?;
    set_target_modes(&mut files, &parse_archive_modes(query.modes.as_deref())?);
    let inspection = inspect_resource_files(kind, &files, Some(query.slug.trim()));
    let changelog = match kind {
        ResourceKind::Agent => AGENT_IMPORT_CHANGELOG,
        ResourceKind::Skill => SKILL_IMPORT_CHANGELOG,
        ResourceKind::Plugin | ResourceKind::Workflow | ResourceKind::Command => {
            return Err(
                ConductorError::msg("resource kind does not support this import route").into(),
            )
        }
    };
    let request = CreateResourceRequest {
        kind,
        slug: query.slug,
        name: query.name,
        description: inspection.metadata.description.clone(),
        version: SemanticVersion::initial().to_string(),
        visibility: query.visibility,
        payload: serde_json::json!({ "files": files }),
        changelog: Some(changelog.into()),
    };
    // A structurally safe archive always becomes an editable Draft. Kind
    // diagnostics are returned to Resource Studio and block release there.
    let resource = super::resources::create_imported_resource(&state, &actor, request).await?;
    Ok(Json(ResourceArchiveCreateResponse {
        resource,
        validation: inspection.validation,
    }))
}

pub async fn inspect_plugin_archive(
    AuthUser(actor): AuthUser,
    body: Bytes,
) -> ApiResult<Json<PluginArchiveInspection>> {
    require_author(actor.primary_role)?;
    let files = extract_plugin_archive(body).await?;
    Ok(Json(inspect_plugin_files(&files)))
}

pub async fn create_plugin_archive(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Query(query): Query<CreatePluginArchiveQuery>,
    body: Bytes,
) -> ApiResult<Json<PluginArchiveCreateResponse>> {
    require_author(actor.primary_role)?;
    let files = extract_plugin_archive(body).await?;
    let inspection = inspect_plugin_files(&files);
    if !inspection.validation.valid {
        let codes = inspection
            .validation
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == conductor_domain::DiagnosticSeverity::Error)
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(ConductorError::msg(format!("validation_failed:{codes}")).into());
    }
    let slug = inspection
        .manifest
        .name
        .clone()
        .ok_or_else(|| ConductorError::msg("plugin manifest name is required"))?;
    let version = inspection
        .manifest
        .version
        .clone()
        .ok_or_else(|| ConductorError::msg("plugin manifest version is required"))?;
    let request = CreateResourceRequest {
        kind: ResourceKind::Plugin,
        slug,
        name: query.name,
        description: inspection.manifest.description.clone(),
        version,
        visibility: query.visibility,
        payload: serde_json::json!({ "files": files }),
        changelog: Some(PLUGIN_IMPORT_CHANGELOG.into()),
    };
    let resource = super::resources::create_imported_resource(&state, &actor, request).await?;
    Ok(Json(PluginArchiveCreateResponse {
        resource,
        validation: inspection.validation,
    }))
}

async fn extract_plugin_archive(body: Bytes) -> ApiResult<Vec<DraftFile>> {
    tokio::task::spawn_blocking(move || import_zip(body.to_vec()))
        .await
        .map_err(|_| ConductorError::Internal)?
        .map_err(|message| ConductorError::msg(message).into())
}

async fn extract_resource_archive(body: Bytes) -> ApiResult<Vec<DraftFile>> {
    tokio::task::spawn_blocking(move || import_zip(body.to_vec()))
        .await
        .map_err(|_| ConductorError::Internal)?
        .map_err(|message| ConductorError::msg(message).into())
}

fn inspect_resource_files(
    kind: ResourceKind,
    files: &[DraftFile],
    expected_slug: Option<&str>,
) -> ResourceArchiveInspection {
    let source = archive_source_metadata(kind, files);
    let validation_slug = expected_slug
        .filter(|slug| !slug.is_empty())
        .map(str::to_string)
        .or_else(|| source.slug.clone())
        .unwrap_or_default();
    ResourceArchiveInspection {
        kind,
        metadata: ResourceArchiveMetadata {
            slug: source.slug,
            version: source.version,
            description: source.description,
            primary_source: source.primary_source,
        },
        validation: validate_draft(kind, &validation_slug, 0, files),
        file_count: files.len(),
        total_uncompressed_bytes: files
            .iter()
            .map(|file| u64::try_from(file.content.len()).unwrap_or(u64::MAX))
            .sum(),
    }
}

fn inspect_plugin_files(files: &[DraftFile]) -> PluginArchiveInspection {
    let manifest = files
        .iter()
        .find(|file| file.path == "plugin.json")
        .and_then(|file| serde_json::from_str::<serde_json::Value>(&file.content).ok())
        .and_then(|value| value.as_object().cloned())
        .map(|object| PluginArchiveManifest {
            name: manifest_string(&object, "name"),
            version: manifest_string(&object, "version"),
            description: manifest_string(&object, "description"),
        })
        .unwrap_or(PluginArchiveManifest {
            name: None,
            version: None,
            description: None,
        });
    let slug = manifest.name.as_deref().unwrap_or_default();
    let validation = validate_draft(ResourceKind::Plugin, slug, 0, files);
    PluginArchiveInspection {
        manifest,
        validation,
        file_count: files.len(),
        total_uncompressed_bytes: files
            .iter()
            .map(|file| u64::try_from(file.content.len()).unwrap_or(u64::MAX))
            .sum(),
        skill_count: files
            .iter()
            .filter(|file| file.path.starts_with("skills/") && file.path.ends_with("/SKILL.md"))
            .count(),
    }
}

fn manifest_string(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub async fn import_archive(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Query(query): Query<ImportQuery>,
    body: Bytes,
) -> ApiResult<Json<DraftImportResponse>> {
    let resource = authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    let files = tokio::task::spawn_blocking(move || import_zip(body.to_vec()))
        .await
        .map_err(|_| ConductorError::Internal)?
        .map_err(ConductorError::msg)?;
    let tree = replace_draft_files(&state, resource_id, files, query.draft_revision)
        .await?
        .0;
    let validation = validate_draft(resource.kind, &resource.slug, tree.revision, &tree.files);
    Ok(Json(DraftImportResponse { tree, validation }))
}

pub async fn validate(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
) -> ApiResult<Json<ResourceValidation>> {
    let resource = authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    let tree = current_draft(&state, resource_id).await?;
    Ok(Json(validate_draft(
        resource.kind,
        &resource.slug,
        tree.revision,
        &tree.files,
    )))
}

pub async fn release(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Json(mut request): Json<ReleaseResourceRequest>,
) -> ApiResult<Json<ReleaseResourceResult>> {
    let resource = authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    request.beta_member_ids.sort();
    request.beta_member_ids.dedup();
    if (request.channel == conductor_domain::ReleaseChannel::Beta
        && request.beta_member_ids.is_empty())
        || request.beta_member_ids.len() > 500
        || !state
            .db
            .users()
            .all_users_active(&request.beta_member_ids)
            .await?
    {
        return Err(ConductorError::msg("beta audience must contain active members").into());
    }
    if let Some(minimum) = request
        .minimum_evoflux_version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        SemanticVersion::from_str(minimum.trim()).map_err(|_| {
            ConductorError::msg("minimum_evoflux_version must follow strict SemVer 2.0")
        })?;
    }
    let tree = current_draft(&state, resource_id).await?;
    if tree.revision != request.draft_revision {
        return Err(ConductorError::Conflict("draft_revision_conflict".into()).into());
    }
    let candidate = release_candidate(resource.highest_version.as_deref(), &request)?;
    let release_files = if resource.kind == ResourceKind::Plugin {
        if request.version_mode == VersionMode::Manual {
            let manifest_version = tree
                .files
                .iter()
                .find(|file| file.path == "plugin.json")
                .and_then(|file| serde_json::from_str::<serde_json::Value>(&file.content).ok())
                .and_then(|value| {
                    value
                        .get("version")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                });
            if manifest_version.as_deref() != Some(candidate.as_str()) {
                return Err(ConductorError::msg("manifest_version_mismatch").into());
            }
        }
        versioned_plugin_files(&tree.files, &candidate)
            .map_err(|diagnostic| ConductorError::msg(diagnostic.message))?
    } else {
        tree.files.clone()
    };
    let validation = validate_draft(resource.kind, &resource.slug, tree.revision, &release_files);
    if !validation.valid {
        let codes = validation
            .diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(ConductorError::msg(format!("validation_failed:{codes}")).into());
    }
    let artifact = state
        .artifacts
        .put_bundle(&release_files)
        .await
        .map_err(|error| ConductorError::msg(format!("object storage write failed: {error}")))?;
    let artifact_media_type = resource_archive_media_type(resource.kind);
    let updated_payload = resource_storage_payload(
        resource.kind,
        &resource.slug,
        &candidate,
        &artifact.key,
        &artifact.sha256,
        artifact.size,
        artifact_media_type,
        &release_files,
    );
    let content = ReleaseContent {
        sha256: artifact.sha256,
        size: artifact.size,
        artifact_key: Some(artifact.key),
        updated_payload: Some(
            serde_json::to_string(&updated_payload).map_err(|_| ConductorError::Internal)?,
        ),
    };
    match state
        .db
        .resources()
        .release(resource_id, &request, &content, actor.id)
        .await
    {
        Ok(result) => Ok(Json(result)),
        Err(ReleaseResourceError::NotFound) => {
            Err(ConductorError::NotFound("resource".into()).into())
        }
        Err(ReleaseResourceError::Conflict) => {
            Err(ConductorError::Conflict("version_conflict".into()).into())
        }
        Err(ReleaseResourceError::InvalidVersion) => Err(ConductorError::msg(
            "manual version must be valid and greater than the current head",
        )
        .into()),
        Err(ReleaseResourceError::Database(error)) => Err(ApiError::from(error)),
    }
}

#[derive(Debug, Deserialize)]
pub struct ChangeQuery {
    cursor: Option<String>,
    limit: Option<u32>,
}

pub async fn changes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ChangeQuery>,
) -> ApiResult<Json<ResourceChangePage>> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::SubscribeResources).await?;
    let project_id = state
        .db
        .instance()
        .project_id()
        .await?
        .ok_or_else(|| ConductorError::NotFound("project".into()))?;
    let after = decode_cursor(
        &state,
        query.cursor.as_deref(),
        project_id,
        principal.user.id,
    )
    .await?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_CHANGE_LIMIT)
        .clamp(1, MAX_CHANGE_LIMIT);
    let rows = state
        .db
        .resources()
        .change_sequences(
            project_id,
            principal.user.id,
            after,
            limit.saturating_add(1),
        )
        .await?;
    let has_more = rows.len() > limit as usize;
    let page_rows = rows.into_iter().take(limit as usize).collect::<Vec<_>>();
    let mut changes = Vec::with_capacity(page_rows.len());
    for (_, resource_id) in &page_rows {
        if let Some(version) = state
            .db
            .resources()
            .effective_version(*resource_id, principal.user.id)
            .await?
        {
            changes.push(to_change(version));
        } else if let Some(resource) = state.db.resources().find_by_id(*resource_id).await? {
            changes.push(ResourceChange {
                project_id,
                resource_id: resource.id,
                version_id: None,
                kind: resource.kind,
                slug: resource.slug,
                version: None,
                description: resource.description,
                changelog: None,
                version_history: Vec::new(),
                release_channel: None,
                sha256: None,
                size: 0,
                bundle_schema_version: None,
                artifact_sha256: None,
                tree_sha256: None,
                artifact_media_type: None,
                file_count: None,
                minimum_evoflux_version: None,
                trust_required: false,
                tombstone: true,
            });
        }
    }
    let next_sequence = page_rows
        .last()
        .map(|(sequence, _)| *sequence)
        .unwrap_or(after);
    Ok(Json(ResourceChangePage {
        schema_version: CHANGE_SCHEMA_VERSION,
        project_id,
        next_cursor: encode_cursor(&state, project_id, principal.user.id, next_sequence).await?,
        has_more,
        changes,
    }))
}

pub async fn version_payload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_id, version_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<EffectiveResourceVersion>> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::SubscribeResources).await?;
    let mut version = state
        .db
        .resources()
        .effective_version(resource_id, principal.user.id)
        .await?
        .filter(|version| version.version_id == version_id)
        .ok_or(ConductorError::Forbidden)?;
    let key = version
        .artifact_key
        .as_deref()
        .ok_or(ConductorError::Forbidden)?;
    let bytes = state
        .artifacts
        .read(key)
        .await
        .map_err(|error| ConductorError::msg(format!("object storage read failed: {error}")))?;
    if hex::encode(Sha256::digest(&bytes)) != version.sha256 {
        return Err(ConductorError::msg("release artifact integrity check failed").into());
    }
    let files = tokio::task::spawn_blocking(move || import_zip(bytes))
        .await
        .map_err(|_| ConductorError::Internal)?
        .map_err(ConductorError::msg)?;
    version.payload["files"] = serde_json::to_value(files).map_err(|_| ConductorError::Internal)?;
    Ok(Json(version))
}

/// Git-style have/want negotiation for the complete member-specific resource
/// checkout. The response contains changed tree entries and only the immutable
/// artifact objects the client does not already have.
pub async fn fetch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ResourceFetchRequest>,
) -> ApiResult<Json<ResourceFetchResponse>> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::SubscribeResources).await?;
    let project_id = state
        .db
        .instance()
        .project_id()
        .await?
        .ok_or_else(|| ConductorError::NotFound("project".into()))?;
    if !state
        .db
        .client_installations()
        .belongs_to(request.installation_id, project_id, principal.user.id)
        .await?
    {
        return Err(ConductorError::Forbidden.into());
    }
    if request.have.len() > MAX_FETCH_HAVE {
        return Err(ConductorError::msg("resource fetch is limited to 5000 have entries").into());
    }
    if request
        .have_commit
        .as_deref()
        .is_some_and(|value| !is_lower_hex_sha256(value))
    {
        return Err(ConductorError::msg("have_commit must be a lowercase SHA-256").into());
    }

    let mut have = HashMap::with_capacity(request.have.len());
    let mut have_artifacts = HashSet::with_capacity(request.have.len());
    for item in &request.have {
        if !is_lower_hex_sha256(&item.artifact_sha256) {
            return Err(
                ConductorError::msg("have artifact_sha256 must be a lowercase SHA-256").into(),
            );
        }
        if have.insert(item.resource_id, item).is_some() {
            return Err(ConductorError::msg("duplicate resource_id in fetch have list").into());
        }
        have_artifacts.insert(item.artifact_sha256.as_str());
    }

    let mut stable = None;
    for _ in 0..MAX_FETCH_STABILIZE_ATTEMPTS {
        let before = state
            .db
            .resources()
            .max_change_sequence(project_id, principal.user.id)
            .await?;
        let versions = state
            .db
            .resources()
            .list_effective_versions(principal.user.id)
            .await?;
        let after = state
            .db
            .resources()
            .max_change_sequence(project_id, principal.user.id)
            .await?;
        if before == after {
            stable = Some((versions, after));
            break;
        }
    }
    let Some((versions, sequence)) = stable else {
        return Err(ConductorError::Conflict(
            "resource head changed while planning fetch; retry".into(),
        )
        .into());
    };

    let mut current = Vec::new();
    for version in versions {
        let Some(kind) = ResourceBundleKind::from_resource_kind(version.kind) else {
            continue;
        };
        let bundle = version.bundle.ok_or_else(|| {
            ConductorError::Conflict(format!(
                "resource {}/{} has no portable bundle artifact",
                kind.as_str(),
                version.slug
            ))
        })?;
        if bundle.artifact_sha256 != version.sha256 || bundle.artifact_size != version.size {
            return Err(ConductorError::Conflict(format!(
                "resource {}/{} bundle metadata does not match its immutable version",
                kind.as_str(),
                version.slug
            ))
            .into());
        }
        current.push(ResourceFetchEntry {
            resource_id: version.resource_id,
            version_id: version.version_id,
            kind,
            slug: version.slug,
            version: version.version,
            release_channel: version.release_channel,
            minimum_evoflux_version: version.minimum_evoflux_version,
            trust_required: version.kind == ResourceKind::Plugin,
            bundle,
        });
    }
    current.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then_with(|| left.slug.cmp(&right.slug))
            .then_with(|| left.resource_id.cmp(&right.resource_id))
    });

    let tree_sha256 = fetch_tree_sha256(&current);
    let commit_id = fetch_commit_id(&tree_sha256);
    let up_to_date = request.have_commit.as_deref() == Some(commit_id.as_str());
    let current_ids = current
        .iter()
        .map(|entry| entry.resource_id)
        .collect::<HashSet<_>>();
    let mut entries = Vec::new();
    let mut objects = BTreeMap::new();
    let mut tombstones = Vec::new();

    if !up_to_date {
        for entry in &current {
            let unchanged = have.get(&entry.resource_id).is_some_and(|item| {
                item.version_id == entry.version_id
                    && item.artifact_sha256 == entry.bundle.artifact_sha256
            });
            if unchanged {
                continue;
            }
            entries.push(entry.clone());
            if !have_artifacts.contains(entry.bundle.artifact_sha256.as_str()) {
                objects
                    .entry(entry.bundle.artifact_sha256.clone())
                    .or_insert_with(|| ResourceFetchObject {
                        artifact_sha256: entry.bundle.artifact_sha256.clone(),
                        size: entry.bundle.artifact_size,
                        media_type: entry.bundle.artifact_media_type.clone(),
                        href: format!(
                            "/api/v1/resources/{}/versions/{}/artifact",
                            entry.resource_id, entry.version_id
                        ),
                    });
            }
        }
        tombstones.extend(
            have.keys()
                .filter(|resource_id| !current_ids.contains(resource_id))
                .map(|resource_id| ResourceFetchTombstone {
                    resource_id: *resource_id,
                }),
        );
        tombstones.sort_by_key(|item| item.resource_id);
    }

    Ok(Json(ResourceFetchResponse {
        schema_version: FETCH_SCHEMA_VERSION,
        project_id,
        base_commit: request.have_commit,
        commit: ResourceFetchCommit {
            id: commit_id,
            tree_sha256,
            sequence,
        },
        up_to_date,
        entries,
        tombstones,
        objects: objects.into_values().collect(),
    }))
}

pub async fn artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_id, version_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::SubscribeResources).await?;
    let version = state
        .db
        .resources()
        .effective_version(resource_id, principal.user.id)
        .await?
        .filter(|version| version.version_id == version_id)
        .ok_or(ConductorError::Forbidden)?;
    let etag = format!("\"sha256:{}\"", version.sha256);
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == etag))
    {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response.headers_mut().insert(
            header::ETAG,
            HeaderValue::from_str(&etag).map_err(|_| ConductorError::Internal)?,
        );
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=31536000, immutable"),
        );
        return Ok(response);
    }
    let key = version.artifact_key.ok_or(ConductorError::Forbidden)?;
    let bytes = state
        .artifacts
        .read(&key)
        .await
        .map_err(|_| ConductorError::Internal)?;
    if hex::encode(Sha256::digest(&bytes)) != version.sha256 {
        return Err(ConductorError::Internal.into());
    }
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(resource_archive_media_type(version.kind)),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=resource.evoresource.zip"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&etag).map_err(|_| ConductorError::Internal)?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    Ok(response)
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn hash_fetch_frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn fetch_tree_sha256(entries: &[ResourceFetchEntry]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"evoflux-resource-tree-v1\0");
    for entry in entries {
        hash_fetch_frame(&mut hasher, entry.kind.as_str().as_bytes());
        hash_fetch_frame(&mut hasher, entry.resource_id.to_string().as_bytes());
        hash_fetch_frame(&mut hasher, entry.version_id.to_string().as_bytes());
        hash_fetch_frame(&mut hasher, entry.slug.as_bytes());
        hash_fetch_frame(&mut hasher, entry.version.as_bytes());
        hash_fetch_frame(&mut hasher, entry.release_channel.as_str().as_bytes());
        hash_fetch_frame(&mut hasher, entry.bundle.artifact_sha256.as_bytes());
        hash_fetch_frame(&mut hasher, entry.bundle.tree_sha256.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn fetch_commit_id(tree_sha256: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"evoflux-resource-commit-v1\0");
    hash_fetch_frame(&mut hasher, tree_sha256.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn inventory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ResourceInventoryRequest>,
) -> ApiResult<Json<ResourceInventoryResponse>> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::SyncInventory).await?;
    if request.items.len() > 500 {
        return Err(ConductorError::msg("inventory batch is limited to 500 items").into());
    }
    let project_id = state
        .db
        .instance()
        .project_id()
        .await?
        .ok_or_else(|| ConductorError::NotFound("project".into()))?;
    if !state
        .db
        .client_installations()
        .belongs_to(request.installation_id, project_id, principal.user.id)
        .await?
    {
        return Err(ConductorError::Forbidden.into());
    }
    let known = state
        .db
        .resources()
        .visible_resource_ids(principal.user.id)
        .await?;
    let requested = request
        .items
        .iter()
        .map(|item| item.resource_id)
        .collect::<HashSet<_>>();
    if !requested.is_subset(&known) {
        return Err(ConductorError::Forbidden.into());
    }
    let accepted = state
        .db
        .resources()
        .upsert_inventory(project_id, &request)
        .await?;
    Ok(Json(ResourceInventoryResponse { accepted }))
}

async fn authorize_resource(
    state: &AppState,
    actor_id: Uuid,
    role: PrimaryRole,
    resource_id: Uuid,
) -> ApiResult<conductor_domain::ManagedResource> {
    require_author(role)?;
    let resource = state
        .db
        .resources()
        .find_by_id(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
    if role != PrimaryRole::Admin && resource.owner_user_id != Some(actor_id) {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(resource)
}

fn require_author(role: PrimaryRole) -> ApiResult<()> {
    if matches!(role, PrimaryRole::Admin | PrimaryRole::Contribute) {
        Ok(())
    } else {
        Err(ConductorError::Forbidden.into())
    }
}

fn parse_kind(value: &str) -> ApiResult<ResourceKind> {
    ResourceKind::parse(value).ok_or_else(|| ConductorError::msg("unknown resource kind").into())
}

fn parse_import_kind(value: &str) -> ApiResult<ResourceKind> {
    match parse_kind(value)? {
        kind @ (ResourceKind::Agent | ResourceKind::Skill) => Ok(kind),
        ResourceKind::Plugin | ResourceKind::Workflow | ResourceKind::Command => {
            Err(ConductorError::msg("only Agent and Skill ZIP imports use this route").into())
        }
    }
}

fn release_candidate(highest: Option<&str>, request: &ReleaseResourceRequest) -> ApiResult<String> {
    let highest = highest
        .map(SemanticVersion::from_str)
        .transpose()
        .map_err(|_| ConductorError::Internal)?;
    match request.version_mode {
        VersionMode::Auto => Ok(highest
            .as_ref()
            .map(SemanticVersion::next_patch)
            .unwrap_or_else(SemanticVersion::initial)
            .to_string()),
        VersionMode::Manual => {
            let candidate = request
                .manual_version
                .as_deref()
                .ok_or_else(|| ConductorError::msg("manual_version is required"))?;
            let parsed = SemanticVersion::from_str(candidate)
                .map_err(|_| ConductorError::msg("manual_version must follow strict SemVer 2.0"))?;
            if highest.as_ref().is_some_and(|head| parsed <= *head) {
                return Err(ConductorError::msg(
                    "manual_version must be greater than the current head",
                )
                .into());
            }
            Ok(parsed.to_string())
        }
    }
}

fn to_change(version: EffectiveResourceVersion) -> ResourceChange {
    let trust_required = version.kind == ResourceKind::Plugin;
    let bundle_schema_version = version.bundle.as_ref().map(|bundle| bundle.schema_version);
    let artifact_sha256 = version
        .bundle
        .as_ref()
        .map(|bundle| bundle.artifact_sha256.clone());
    let tree_sha256 = version
        .bundle
        .as_ref()
        .map(|bundle| bundle.tree_sha256.clone());
    let artifact_media_type = version
        .bundle
        .as_ref()
        .map(|bundle| bundle.artifact_media_type.clone());
    let file_count = version
        .bundle
        .as_ref()
        .map(|bundle| u32::try_from(bundle.files.len()).unwrap_or(u32::MAX));
    ResourceChange {
        project_id: version.project_id,
        resource_id: version.resource_id,
        version_id: Some(version.version_id),
        kind: version.kind,
        slug: version.slug,
        version: Some(version.version),
        description: version.description,
        changelog: version.changelog,
        version_history: version.version_history,
        release_channel: Some(version.release_channel),
        sha256: Some(version.sha256),
        size: version.size,
        bundle_schema_version,
        artifact_sha256,
        tree_sha256,
        artifact_media_type,
        file_count,
        minimum_evoflux_version: version.minimum_evoflux_version,
        trust_required,
        tombstone: false,
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CursorPayload {
    schema: u8,
    project_id: Uuid,
    user_id: Uuid,
    sequence: i64,
}

async fn encode_cursor(
    state: &AppState,
    project_id: Uuid,
    user_id: Uuid,
    sequence: i64,
) -> ApiResult<String> {
    let payload = serde_json::to_vec(&CursorPayload {
        schema: CHANGE_SCHEMA_VERSION,
        project_id,
        user_id,
        sequence,
    })
    .map_err(|_| ConductorError::Internal)?;
    let secret = state
        .db
        .instance()
        .jwt_secret()
        .await?
        .ok_or(ConductorError::Internal)?;
    let signature = sign_cursor(&payload, secret.as_bytes());
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(signature)
    ))
}

async fn decode_cursor(
    state: &AppState,
    cursor: Option<&str>,
    project_id: Uuid,
    user_id: Uuid,
) -> ApiResult<i64> {
    let Some(cursor) = cursor.filter(|cursor| !cursor.is_empty()) else {
        return Ok(0);
    };
    let (payload, signature) = cursor
        .split_once('.')
        .ok_or_else(|| ConductorError::msg("invalid cursor"))?;
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| ConductorError::msg("invalid cursor"))?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| ConductorError::msg("invalid cursor"))?;
    let secret = state
        .db
        .instance()
        .jwt_secret()
        .await?
        .ok_or(ConductorError::Internal)?;
    if !verify_cursor_signature(&payload, &signature, secret.as_bytes()) {
        return Err(ConductorError::msg("invalid cursor signature").into());
    }
    let decoded: CursorPayload =
        serde_json::from_slice(&payload).map_err(|_| ConductorError::msg("invalid cursor"))?;
    if decoded.schema != CHANGE_SCHEMA_VERSION
        || decoded.project_id != project_id
        || decoded.user_id != user_id
        || decoded.sequence < 0
    {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(decoded.sequence)
}

fn sign_cursor(payload: &[u8], secret: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts keys of any size");
    mac.update(payload);
    mac.finalize().into_bytes().to_vec()
}

fn verify_cursor_signature(payload: &[u8], signature: &[u8], secret: &[u8]) -> bool {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts keys of any size");
    mac.update(payload);
    mac.verify_slice(signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conductor_domain::{FileManifestEntry, ReleaseChannel, ResourceBundle, ResourceBundleKind};

    #[test]
    fn archive_modes_accept_aim_and_keep_canonical_order() {
        let modes = match parse_archive_modes(Some("aim,work")) {
            Ok(modes) => modes,
            Err(_) => panic!("AIM mode should be accepted"),
        };
        assert_eq!(
            modes,
            vec![ResourceTargetMode::Work, ResourceTargetMode::Aim]
        );
    }

    #[test]
    fn smart_fetch_commit_has_a_cross_language_golden_vector() {
        let entry = ResourceFetchEntry {
            resource_id: Uuid::from_u128(1),
            version_id: Uuid::from_u128(2),
            kind: ResourceBundleKind::Skill,
            slug: "audit".into(),
            version: "1.2.3".into(),
            release_channel: ReleaseChannel::Published,
            bundle: ResourceBundle {
                schema_version: ResourceBundle::SCHEMA_VERSION,
                kind: ResourceBundleKind::Skill,
                slug: "audit".into(),
                version: "1.2.3".into(),
                artifact_sha256: "a".repeat(64),
                artifact_size: 42,
                artifact_media_type: "application/vnd.evoflux.resource+zip".into(),
                tree_sha256: "b".repeat(64),
                files: Vec::new(),
            },
            minimum_evoflux_version: None,
            trust_required: false,
        };

        let tree = fetch_tree_sha256(&[entry]);
        assert_eq!(
            tree,
            "43a48be42482e92625801c5b1abdf7093128a0a96b3c3e886c73380d76045237"
        );
        assert_eq!(
            fetch_commit_id(&tree),
            "7e35c6857cf1f439057ca31d8692f9cc3d29a0e06ba0d7c0d1d2cedc618febdd"
        );
    }

    #[test]
    fn change_descriptor_exposes_bundle_without_removing_legacy_hash() {
        let bundle = ResourceBundle {
            schema_version: ResourceBundle::SCHEMA_VERSION,
            kind: ResourceBundleKind::Agent,
            slug: "reviewer".into(),
            version: "1.0.0".into(),
            artifact_sha256: "a".repeat(64),
            artifact_size: 100,
            artifact_media_type: "application/vnd.evoflux.resource+json".into(),
            tree_sha256: "b".repeat(64),
            files: vec![FileManifestEntry {
                path: "reviewer.md".into(),
                sha256: "c".repeat(64),
                size: 12,
                media_type: "text/markdown".into(),
                executable: false,
            }],
        };
        let change = to_change(EffectiveResourceVersion {
            project_id: Uuid::nil(),
            resource_id: Uuid::nil(),
            version_id: Uuid::nil(),
            kind: ResourceKind::Agent,
            slug: "reviewer".into(),
            version: "1.0.0".into(),
            description: None,
            changelog: None,
            version_history: Vec::new(),
            release_channel: ReleaseChannel::Published,
            payload: serde_json::json!({}),
            sha256: "legacy".into(),
            size: 100,
            artifact_key: None,
            bundle: Some(bundle),
            minimum_evoflux_version: None,
        });

        assert_eq!(change.sha256.as_deref(), Some("legacy"));
        assert_eq!(change.bundle_schema_version, Some(2));
        assert_eq!(change.artifact_sha256, Some("a".repeat(64)));
        assert_eq!(change.tree_sha256, Some("b".repeat(64)));
        assert_eq!(change.file_count, Some(1));
    }
}
