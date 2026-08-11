use axum::{extract::State, Json};
use conductor_auth::{hash_password_async, validate_oidc_redirect_uri, validate_oidc_url};
use conductor_domain::{ConductorError, SetupRequest, SetupStatus, SsoProvider};
use rand::RngCore;

use crate::core::error::ApiResult;
use crate::core::state::AppState;

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
    if req.admin_display_name.trim().is_empty() {
        return Err(ConductorError::msg("admin_display_name is required").into());
    }
    if req.admin_password.len() < 12 {
        return Err(ConductorError::msg("admin_password must be at least 12 characters").into());
    }
    if req.bind_port == 0 {
        return Err(ConductorError::msg("bind_port is required").into());
    }
    if let Some(public_url) = req
        .public_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
    {
        validate_oidc_url(public_url, "public URL")?;
    }

    if let Some(sso) = &req.sso {
        if sso.enabled {
            if sso.provider == SsoProvider::Github {
                return Err(ConductorError::msg(
                    "GitHub OAuth is not an OIDC provider and is not supported yet",
                )
                .into());
            }
            if sso.client_id.as_ref().is_none_or(|s| s.trim().is_empty()) {
                return Err(
                    ConductorError::msg("SSO client_id is required when SSO is enabled").into(),
                );
            }
            if sso
                .client_secret
                .as_ref()
                .is_none_or(|s| s.trim().is_empty())
            {
                return Err(ConductorError::msg(
                    "SSO client_secret is required when SSO is enabled",
                )
                .into());
            }
            if sso.issuer_url.as_ref().is_none_or(|s| s.trim().is_empty()) {
                return Err(
                    ConductorError::msg("SSO issuer_url is required when SSO is enabled").into(),
                );
            }
            validate_oidc_url(sso.issuer_url.as_deref().unwrap_or_default(), "issuer")?;
            if sso
                .redirect_uri
                .as_ref()
                .is_none_or(|s| s.trim().is_empty())
            {
                return Err(ConductorError::msg(
                    "SSO redirect_uri is required when SSO is enabled",
                )
                .into());
            }
            validate_oidc_redirect_uri(sso.redirect_uri.as_deref().unwrap_or_default())?;
            if sso
                .scopes
                .as_ref()
                .is_some_and(|scopes| !scopes.iter().any(|scope| scope == "openid"))
            {
                return Err(ConductorError::msg("SSO scopes must include openid").into());
            }
        }
    }

    let mut req = req;
    req.project_name = req.project_name.trim().to_string();
    req.admin_email = req.admin_email.trim().to_lowercase();
    req.admin_display_name = req.admin_display_name.trim().to_string();
    req.bind_host = req.bind_host.trim().to_string();
    req.public_url = req
        .public_url
        .take()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty());
    if let Some(sso) = req.sso.as_mut() {
        sso.issuer_url = sso
            .issuer_url
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        sso.client_id = sso
            .client_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        sso.redirect_uri = sso
            .redirect_uri
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if sso.enabled && sso.scopes.as_ref().is_none_or(|s| s.is_empty()) {
            sso.scopes = Some(conductor_auth::default_scopes(sso.provider));
        }
    }

    let password_hash = hash_password_async(req.admin_password.clone()).await?;
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
