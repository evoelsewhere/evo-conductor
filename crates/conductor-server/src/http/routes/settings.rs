use axum::{extract::State, Json};
use conductor_auth::{validate_oidc_redirect_uri, validate_oidc_url};
use conductor_domain::{
    ConductorError, ProjectBranding, ProjectSettings, RealtimeSettings, SsoProvider,
    UpdateInstanceRequest, UpdateNetworkRequest, UpdateSsoRequest,
};
use conductor_storage::repos::SsoConfigUpdate;

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
        logo_url: instance.logo_url,
    }))
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
    let realtime_config = state.realtime.config();
    Ok(Json(ProjectSettings {
        project_name: instance.project_name,
        display_name: instance.display_name,
        bind_host: instance.bind_host,
        bind_port: instance.bind_port,
        public_url: instance.public_url,
        logo_url: instance.logo_url,
        realtime: RealtimeSettings {
            max_connections: realtime_config.max_connections as u32,
            max_connections_per_secret: realtime_config.max_connections_per_secret as u32,
            heartbeat_seconds: realtime_config.heartbeat_seconds as u32,
        },
        sso,
    }))
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
        .update_instance(
            req.project_name.as_deref().map(str::trim),
            req.display_name.as_deref(),
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
