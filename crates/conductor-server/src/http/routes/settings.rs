use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use conductor_auth::{validate_oidc_redirect_uri, validate_oidc_url};
use conductor_domain::{
    CollectionLevel, ConductorError, DataPolicySettings, ProjectBranding, ProjectSettings,
    RealtimeSettings, SsoProvider, StorageMigrationResult, UpdateDataPolicyRequest,
    UpdateInstanceRequest, UpdateNetworkRequest, UpdateSsoRequest, UpdateStorageRequest,
};
use conductor_storage::repos::{LogoArtifact, SsoConfigUpdate};
use sha2::{Digest, Sha256};

const MAX_LOGO_BYTES: usize = 512 * 1024;
const MAX_PROJECT_DESCRIPTION_CHARS: usize = 500;

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

/// Lightweight branding for every authenticated member (sidebar / topbar).
pub async fn get_project(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<ProjectBranding>> {
    let instance = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?;
    Ok(Json(ProjectBranding {
        project_name: instance.project_name,
        display_name: instance.display_name,
        description: instance.description,
        logo_url: instance.logo_url,
    }))
}

pub async fn get_project_logo(State(state): State<AppState>) -> ApiResult<Response> {
    let logo = state
        .db
        .instance()
        .logo_artifact()
        .await?
        .ok_or_else(|| ConductorError::NotFound("project logo".into()))?;
    let bytes = state
        .artifacts
        .read(&logo.key)
        .await
        .map_err(|error| ConductorError::msg(format!("object storage read failed: {error}")))?;
    if hex::encode(Sha256::digest(&bytes)) != logo.sha256
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != logo.size
    {
        return Err(ConductorError::msg("project logo integrity check failed").into());
    }
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&logo.media_type).map_err(|_| ConductorError::Internal)?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

pub async fn get_settings(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<ProjectSettings>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }
    let instance = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?;
    let sso = state.db.instance().sso_config().await?;
    let storage = state.db.instance().storage_settings().await?;
    let collection_level = CollectionLevel::parse(&state.db.instance().collection_level().await?);
    let realtime_config = state.realtime.config();
    Ok(Json(ProjectSettings {
        project_name: instance.project_name,
        display_name: instance.display_name,
        description: instance.description,
        bind_host: instance.bind_host,
        bind_port: instance.bind_port,
        public_url: instance.public_url,
        logo_url: instance.logo_url,
        realtime: RealtimeSettings {
            max_connections: realtime_config.max_connections as u32,
            max_connections_per_secret: realtime_config.max_connections_per_secret as u32,
            heartbeat_seconds: realtime_config.heartbeat_seconds as u32,
        },
        data_policy: DataPolicySettings { collection_level },
        sso,
        storage,
    }))
}

pub async fn update_data_policy(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<UpdateDataPolicyRequest>,
) -> ApiResult<Json<ProjectSettings>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }
    state
        .db
        .instance()
        .update_collection_level(request.collection_level.as_str())
        .await?;
    get_settings(State(state), AuthUser(user)).await
}

pub async fn update_settings(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<UpdateInstanceRequest>,
) -> ApiResult<Json<ProjectSettings>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }
    if let Some(ref name) = req.project_name {
        if name.trim().is_empty() {
            return Err(ConductorError::msg("project_name cannot be empty").into());
        }
    }
    if req
        .description
        .as_deref()
        .is_some_and(|value| value.trim().chars().count() > MAX_PROJECT_DESCRIPTION_CHARS)
    {
        return Err(ConductorError::msg(format!(
            "description must be at most {MAX_PROJECT_DESCRIPTION_CHARS} characters"
        ))
        .into());
    }
    if let Some(public_url) = req
        .public_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
    {
        validate_oidc_url(public_url, "public URL")?;
    }
    if req
        .logo_url
        .as_deref()
        .is_some_and(|value| value.trim_start().starts_with("data:"))
    {
        return Err(ConductorError::msg(
            "inline logo data is not allowed; upload the image through /settings/logo",
        )
        .into());
    }

    state
        .db
        .instance()
        .update_instance(
            req.project_name.as_deref().map(str::trim),
            req.display_name.as_deref(),
            req.description.as_deref(),
            req.public_url.as_deref(),
            req.logo_url.as_deref(),
        )
        .await?
        .ok_or(ConductorError::SetupRequired)?;

    get_settings(State(state), AuthUser(user)).await
}

pub async fn update_network(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<UpdateNetworkRequest>,
) -> ApiResult<Json<ProjectSettings>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }
    let bind_host = req.bind_host.trim();
    if bind_host.is_empty() {
        return Err(ConductorError::msg("bind_host cannot be empty").into());
    }
    if req.bind_port == 0 {
        return Err(ConductorError::msg("bind_port must be between 1 and 65535").into());
    }
    if req.realtime.max_connections == 0 || req.realtime.max_connections_per_secret == 0 {
        return Err(ConductorError::msg("realtime connection limits must be at least 1").into());
    }
    if !(5..=300).contains(&req.realtime.heartbeat_seconds) {
        return Err(ConductorError::msg("heartbeat must be between 5 and 300 seconds").into());
    }
    if let Some(public_url) = req
        .public_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
    {
        validate_oidc_url(public_url, "public URL")?;
    }

    state
        .db
        .instance()
        .update_network(
            bind_host,
            req.bind_port,
            req.public_url.as_deref(),
            &req.realtime,
        )
        .await?;

    // Apply what can change live; the rest takes effect on the next start.
    let mut realtime = state.realtime.config();
    realtime.max_connections = req.realtime.max_connections as usize;
    realtime.max_connections_per_secret = req.realtime.max_connections_per_secret as usize;
    realtime.heartbeat_seconds = u64::from(req.realtime.heartbeat_seconds);
    state.realtime.update_config(realtime);

    get_settings(State(state), AuthUser(user)).await
}

pub async fn update_storage(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<UpdateStorageRequest>,
) -> ApiResult<Json<StorageMigrationResult>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }
    let current = state.db.instance().storage_settings().await?;
    let credential_change = request.storage.git.clear_credential
        || request
            .storage
            .git
            .credential
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    if current == request.storage && !credential_change {
        return Ok(Json(StorageMigrationResult {
            storage: current,
            objects_copied: 0,
            bytes_copied: 0,
        }));
    }
    let mut keys = state.db.resources().object_keys().await?;
    if let Some(logo) = state.db.instance().logo_artifact().await? {
        keys.push(logo.key);
    }
    if current != request.storage && !request.migrate_existing && !keys.is_empty() {
        return Err(ConductorError::Conflict(
            "existing resource objects must be migrated before changing storage".into(),
        )
        .into());
    }

    let settings = request.storage;
    let instance = state.db.instance();
    let stats = state
        .artifacts
        .reconfigure(settings, keys, move |persisted| async move {
            instance
                .update_storage_settings(&persisted)
                .await
                .map_err(anyhow::Error::from)
        })
        .await
        .map_err(|error| ConductorError::msg(format!("storage migration failed: {error}")))?;
    let settings = state.artifacts.settings().await;

    Ok(Json(StorageMigrationResult {
        storage: settings,
        objects_copied: stats.objects_copied,
        bytes_copied: stats.bytes_copied,
    }))
}

pub async fn upload_logo(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<ProjectSettings>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }
    if body.is_empty() || body.len() > MAX_LOGO_BYTES {
        return Err(ConductorError::msg("logo must be between 1 byte and 512 KiB").into());
    }
    let media_type = validated_logo_media_type(&headers, &body)?;
    let artifact =
        state.artifacts.put(&body).await.map_err(|error| {
            ConductorError::msg(format!("object storage write failed: {error}"))
        })?;
    state
        .db
        .instance()
        .update_logo_artifact(Some(&LogoArtifact {
            key: artifact.key,
            sha256: artifact.sha256,
            size: artifact.size,
            media_type: media_type.into(),
        }))
        .await?;
    get_settings(State(state), AuthUser(user)).await
}

pub async fn delete_logo(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<ProjectSettings>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }
    state.db.instance().update_logo_artifact(None).await?;
    get_settings(State(state), AuthUser(user)).await
}

fn validated_logo_media_type(headers: &HeaderMap, bytes: &[u8]) -> ApiResult<&'static str> {
    let declared = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    let observed = if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return Err(ConductorError::msg("logo must be PNG, JPEG or WebP").into());
    };
    if declared.is_some_and(|value| value != observed && value != "application/octet-stream") {
        return Err(ConductorError::msg("logo content type does not match its bytes").into());
    }
    Ok(observed)
}

pub async fn update_sso(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<UpdateSsoRequest>,
) -> ApiResult<Json<conductor_domain::SsoConfig>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }

    let mut req = req;
    req.issuer_url = req
        .issuer_url
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    req.client_id = req
        .client_id
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    req.redirect_uri = req
        .redirect_uri
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let existing = state.db.instance().sso_config().await?;
    if req.enabled {
        if req.provider == SsoProvider::Github {
            return Err(ConductorError::msg(
                "GitHub OAuth is not an OIDC provider and is not supported yet",
            )
            .into());
        }
        if req.client_id.as_ref().is_none_or(|s| s.trim().is_empty()) {
            return Err(ConductorError::msg("client_id is required when SSO is enabled").into());
        }
        if req.issuer_url.as_ref().is_none_or(|s| s.trim().is_empty()) {
            return Err(ConductorError::msg("issuer_url is required when SSO is enabled").into());
        }
        validate_oidc_url(req.issuer_url.as_deref().unwrap_or_default(), "issuer")?;
        if req
            .redirect_uri
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
        {
            return Err(ConductorError::msg("redirect_uri is required when SSO is enabled").into());
        }
        validate_oidc_redirect_uri(req.redirect_uri.as_deref().unwrap_or_default())?;
        let has_secret = existing.client_secret_set.unwrap_or(false)
            || req
                .client_secret
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty());
        if !has_secret {
            return Err(ConductorError::msg("client_secret is required when enabling SSO").into());
        }
    }

    let scopes = req.scopes.unwrap_or_else(|| {
        if req.provider == existing.provider && !existing.scopes.is_empty() {
            existing.scopes.clone()
        } else {
            conductor_auth::default_scopes(req.provider)
        }
    });
    if req.enabled && !scopes.iter().any(|scope| scope == "openid") {
        return Err(ConductorError::msg("SSO scopes must include openid").into());
    }

    let config = state
        .db
        .instance()
        .update_sso(SsoConfigUpdate {
            enabled: req.enabled,
            provider: req.provider,
            issuer_url: req.issuer_url.as_deref(),
            client_id: req.client_id.as_deref(),
            client_secret: req.client_secret.as_deref(),
            redirect_uri: req.redirect_uri.as_deref(),
            scopes: Some(scopes.as_slice()),
        })
        .await?;

    Ok(Json(config))
}
