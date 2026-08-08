use axum::{extract::State, Json};
use conductor_auth::hash_password;
use conductor_domain::{ConductorError, SetupRequest, SetupStatus};
use rand::RngCore;

use crate::http::error::ApiResult;
use crate::http::state::AppState;

pub async fn status(State(state): State<AppState>) -> ApiResult<Json<SetupStatus>> {
    Ok(Json(state.db.instance().setup_status().await?))
}

pub async fn complete(
    State(state): State<AppState>,
    Json(req): Json<SetupRequest>,
) -> ApiResult<Json<SetupStatus>> {
    if state.db.instance().is_setup_completed().await? {
        return Err(ConductorError::SetupAlreadyCompleted.into());
    }

    if req.project_name.trim().is_empty() {
        return Err(ConductorError::msg("project_name is required").into());
    }
    if req.admin_email.trim().is_empty() || !req.admin_email.contains('@') {
        return Err(ConductorError::msg("valid admin_email is required").into());
    }
    if req.admin_password.len() < 8 {
        return Err(ConductorError::msg("admin_password must be at least 8 characters").into());
    }
    if req.bind_port == 0 {
        return Err(ConductorError::msg("bind_port is required").into());
    }

    if let Some(sso) = &req.sso {
        if sso.enabled {
            if sso.client_id.as_ref().is_none_or(|s| s.trim().is_empty()) {
                return Err(
                    ConductorError::msg("SSO client_id is required when SSO is enabled").into(),
                );
            }
            if sso.client_secret.as_ref().is_none_or(|s| s.trim().is_empty()) {
                return Err(
                    ConductorError::msg("SSO client_secret is required when SSO is enabled").into(),
                );
            }
            if sso.issuer_url.as_ref().is_none_or(|s| s.trim().is_empty()) {
                return Err(
                    ConductorError::msg("SSO issuer_url is required when SSO is enabled").into(),
                );
            }
            if sso.redirect_uri.as_ref().is_none_or(|s| s.trim().is_empty()) {
                return Err(
                    ConductorError::msg("SSO redirect_uri is required when SSO is enabled").into(),
                );
            }
        }
    }

    let mut req = req;
    if let Some(sso) = req.sso.as_mut() {
        if sso.enabled && sso.scopes.as_ref().is_none_or(|s| s.is_empty()) {
            sso.scopes = Some(conductor_auth::default_scopes(sso.provider));
        }
    }

    let password_hash = hash_password(&req.admin_password)?;
    let mut jwt_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut jwt_bytes);
    let jwt_secret = hex::encode(jwt_bytes);

    let client_secret_enc = req
        .sso
        .as_ref()
        .and_then(|s| s.client_secret.clone())
        .filter(|s| !s.is_empty());

    let (_instance, _admin) = state
        .db
        .instance()
        .complete_setup(
            &req,
            &password_hash,
            &jwt_secret,
            client_secret_enc.as_deref(),
        )
        .await?;

    state.set_jwt_secret(jwt_secret).await;

    tracing::info!(
        project = %req.project_name,
        admin = %req.admin_email,
        "setup completed"
    );

    Ok(Json(state.db.instance().setup_status().await?))
}
