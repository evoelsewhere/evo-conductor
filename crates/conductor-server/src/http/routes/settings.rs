use axum::{extract::State, Json};
use conductor_auth::{validate_oidc_redirect_uri, validate_oidc_url};
use conductor_domain::{
    ConductorError, ProjectBranding, ProjectSettings, SsoProvider, UpdateInstanceRequest,
    UpdateSsoRequest,
};
use conductor_storage::repos::SsoConfigUpdate;

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

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
    Ok(Json(ProjectSettings {
        project_name: instance.project_name,
        display_name: instance.display_name,
        bind_host: instance.bind_host,
        bind_port: instance.bind_port,
        public_url: instance.public_url,
        logo_url: instance.logo_url,
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
