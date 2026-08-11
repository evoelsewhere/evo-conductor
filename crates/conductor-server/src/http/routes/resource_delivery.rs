use std::collections::HashSet;
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
    PrimaryRole, ReleaseResourceRequest, ReleaseResourceResult, ResourceChange, ResourceChangePage,
    ResourceInventoryRequest, ResourceInventoryResponse, ResourceKind, ResourceTargetMode,
    ResourceValidation, ResourceVisibility, SaveDraftFileRequest, SecretScope, SemanticVersion,
    VersionMode,
};
use conductor_storage::repos::{DraftWriteError, ReleaseContent, ReleaseResourceError};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::core::error::{ApiError, ApiResult};
use crate::core::resource_authoring::{
    archive_source_metadata, import_zip, safe_relative_path, set_target_modes, starter_files,
    validate_draft, versioned_plugin_files, MAX_EDITABLE_FILE_BYTES,
};
use crate::core::state::AppState;
use crate::http::extractors::{authenticate_connection_secret, AuthUser};

const CHANGE_SCHEMA_VERSION: u8 = 2;
const DEFAULT_CHANGE_LIMIT: u32 = 100;
const MAX_CHANGE_LIMIT: u32 = 500;
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
    let tree = state
        .db
        .resources()
        .draft_tree(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
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
    match state
        .db
        .resources()
        .save_draft_file(resource_id, &path, &request.content, request.draft_revision)
        .await
    {
        Ok(tree) => Ok(Json(tree)),
        Err(DraftWriteError::NotFound) => Err(ConductorError::NotFound("resource".into()).into()),
        Err(DraftWriteError::Conflict) => {
            Err(ConductorError::Conflict("draft_revision_conflict".into()).into())
        }
        Err(DraftWriteError::Database(error)) => Err(ApiError::from(error)),
    }
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
    state
        .db
        .resources()
        .draft_tree(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()).into())
}

async fn replace_draft_files(
    state: &AppState,
    resource_id: Uuid,
    mut files: Vec<DraftFile>,
    draft_revision: u64,
) -> ApiResult<Json<DraftFileTree>> {
    files.sort_by(|left, right| left.path.cmp(&right.path));
    match state
        .db
        .resources()
        .replace_draft_files(resource_id, &files, draft_revision)
        .await
    {
        Ok(tree) => Ok(Json(tree)),
        Err(DraftWriteError::NotFound) => Err(ConductorError::NotFound("resource".into()).into()),
        Err(DraftWriteError::Conflict) => {
            Err(ConductorError::Conflict("draft_revision_conflict".into()).into())
        }
        Err(DraftWriteError::Database(error)) => Err(ApiError::from(error)),
    }
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
            .ok_or_else(|| ConductorError::msg("modes may contain only work and coding"))?;
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
    let tree = match state
        .db
        .resources()
        .replace_draft_files(resource_id, &files, query.draft_revision)
        .await
    {
        Ok(tree) => tree,
        Err(DraftWriteError::NotFound) => {
            return Err(ConductorError::NotFound("resource".into()).into())
        }
        Err(DraftWriteError::Conflict) => {
            return Err(ConductorError::Conflict("draft_revision_conflict".into()).into())
        }
        Err(DraftWriteError::Database(error)) => return Err(ApiError::from(error)),
    };
    let validation = validate_draft(resource.kind, &resource.slug, tree.revision, &tree.files);
    Ok(Json(DraftImportResponse { tree, validation }))
}

pub async fn validate(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
) -> ApiResult<Json<ResourceValidation>> {
    let resource = authorize_resource(&state, actor.id, actor.primary_role, resource_id).await?;
    let tree = state
        .db
        .resources()
        .draft_tree(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
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
    let tree = state
        .db
        .resources()
        .draft_tree(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
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
    let updated_payload = serde_json::to_string(&serde_json::json!({ "files": release_files }))
        .map_err(|_| ConductorError::Internal)?;
    let content = if resource.kind == ResourceKind::Plugin {
        let artifact = state
            .artifacts
            .put_plugin(&release_files)
            .map_err(|_| ConductorError::Internal)?;
        ReleaseContent {
            sha256: artifact.sha256,
            size: artifact.size,
            artifact_key: Some(artifact.key),
            updated_payload: Some(updated_payload),
        }
    } else {
        let bytes = updated_payload.as_bytes();
        ReleaseContent {
            sha256: hex::encode(Sha256::digest(bytes)),
            size: bytes.len().try_into().unwrap_or(u64::MAX),
            artifact_key: None,
            updated_payload: None,
        }
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
    let version = state
        .db
        .resources()
        .effective_version(resource_id, principal.user.id)
        .await?
        .filter(|version| version.version_id == version_id)
        .ok_or(ConductorError::Forbidden)?;
    Ok(Json(version))
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
    let key = version.artifact_key.ok_or(ConductorError::Forbidden)?;
    let bytes = state
        .artifacts
        .read(&key)
        .map_err(|_| ConductorError::Internal)?;
    if hex::encode(Sha256::digest(&bytes)) != version.sha256 {
        return Err(ConductorError::Internal.into());
    }
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.evoflux.plugin+zip"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=resource.evoplugin"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
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
