use axum::{extract::State, Json};
use conductor_domain::{
    ConductorError, ProjectSettings, UpdateInstanceRequest, UpdateSsoRequest,
};

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

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

    state
        .db
        .instance()
        .update_instance(
            req.project_name.as_deref().map(str::trim),
            req.display_name.as_deref(),
            req.public_url.as_deref(),
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

    if req.enabled {
        if req.client_id.as_ref().is_none_or(|s| s.trim().is_empty()) {
            return Err(ConductorError::msg("client_id is required when SSO is enabled").into());
        }
        if req.issuer_url.as_ref().is_none_or(|s| s.trim().is_empty()) {
            return Err(ConductorError::msg("issuer_url is required when SSO is enabled").into());
        }
        if req.redirect_uri.as_ref().is_none_or(|s| s.trim().is_empty()) {
            return Err(ConductorError::msg("redirect_uri is required when SSO is enabled").into());
        }
        let existing = state.db.instance().sso_config().await?;
        let has_secret = existing.client_secret_set.unwrap_or(false)
            || req
                .client_secret
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty());
        if !has_secret {
            return Err(
                ConductorError::msg("client_secret is required when enabling SSO").into(),
            );
        }
    }

    let scopes = req.scopes.unwrap_or_else(|| {
        conductor_auth::default_scopes(req.provider)
    });

    let config = state
        .db
        .instance()
        .update_sso(
            req.enabled,
            req.provider,
            req.issuer_url.as_deref(),
            req.client_id.as_deref(),
            req.client_secret.as_deref(),
            req.redirect_uri.as_deref(),
            Some(scopes.as_slice()),
        )
        .await?;

    Ok(Json(config))
}
