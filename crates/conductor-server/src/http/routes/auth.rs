use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    Json,
};
use chrono::{TimeZone, Utc};
use conductor_auth::{
    begin_authorization, default_scopes, exchange_code, hash_password, verify_password,
};
use conductor_domain::{AuthSession, ChangePasswordRequest, ConductorError, User, UserStatus};
use conductor_storage::repos::SsoLoginError;
use serde::{Deserialize, Serialize};

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct SsoStartResponse {
    pub authorization_url: String,
    pub provider: String,
}

#[derive(Debug, Deserialize)]
pub struct SsoCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<AuthSession>> {
    if !state.db.instance().is_setup_completed().await? {
        return Err(ConductorError::SetupRequired.into());
    }

    let (user, hash) = state
        .db
        .users()
        .find_by_email(req.email.trim())
        .await?
        .ok_or(ConductorError::InvalidCredentials)?;

    if !user.status.can_authenticate() {
        return Err(ConductorError::InvalidCredentials.into());
    }

    let hash = hash.ok_or(ConductorError::InvalidCredentials)?;

    if !verify_password(&req.password, &hash)? {
        return Err(ConductorError::InvalidCredentials.into());
    }

    let user = if user.status == UserStatus::Invited {
        state
            .db
            .users()
            .activate_invited_on_password_login(user.id)
            .await?
    } else {
        user
    };

    Ok(Json(issue_session(&state, user).await?))
}

pub async fn me(AuthUser(user): AuthUser) -> ApiResult<Json<User>> {
    Ok(Json(user))
}

pub async fn change_password(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> ApiResult<Json<AuthSession>> {
    if req.new_password.len() < 12 {
        return Err(ConductorError::msg("new_password must be at least 12 characters").into());
    }

    let (_, hash) = state
        .db
        .users()
        .find_by_email(&user.email)
        .await?
        .ok_or(ConductorError::Unauthorized)?;

    let hash = hash.ok_or_else(|| {
        ConductorError::msg(
            "this account uses SSO and has no local password; ask an admin to reset it",
        )
    })?;
    let current = req.current_password.as_deref().unwrap_or("");
    if !user.must_change_password {
        if current.is_empty() || !verify_password(current, &hash)? {
            return Err(ConductorError::InvalidCredentials.into());
        }
    } else if !current.is_empty() && !verify_password(current, &hash)? {
        return Err(ConductorError::InvalidCredentials.into());
    }

    let new_hash = hash_password(&req.new_password)?;
    state
        .db
        .users()
        .set_password(user.id, &new_hash, false)
        .await?;

    let updated = state
        .db
        .users()
        .find_by_id(user.id)
        .await?
        .ok_or(ConductorError::NotFound("user".into()))?;
    Ok(Json(issue_session(&state, updated).await?))
}

/// Begin Microsoft Entra ID / generic OIDC login.
pub async fn sso_start(State(state): State<AppState>) -> ApiResult<Json<SsoStartResponse>> {
    let runtime = state
        .db
        .instance()
        .sso_runtime()
        .await?
        .ok_or_else(|| ConductorError::msg("SSO is not enabled or incomplete"))?;

    let scopes = if runtime.scopes.is_empty() {
        default_scopes(runtime.provider)
    } else {
        runtime.scopes.clone()
    };

    let req = begin_authorization(
        &runtime.issuer_url,
        &runtime.client_id,
        &runtime.redirect_uri,
        &scopes,
    )
    .await?;

    state.store_oidc_pending(req.state.clone(), req.code_verifier, req.nonce);

    Ok(Json(SsoStartResponse {
        authorization_url: req.authorization_url,
        provider: runtime.provider.as_str().to_string(),
    }))
}

/// OIDC redirect URI — register this in Azure App Registration
/// (e.g. `http://localhost:4700/api/auth/sso/callback`).
pub async fn sso_callback(
    State(state): State<AppState>,
    Query(query): Query<SsoCallbackQuery>,
) -> Result<impl IntoResponse, crate::http::error::ApiError> {
    if let Some(err) = query.error {
        let detail = query.error_description.unwrap_or(err);
        return Err(ConductorError::msg(format!("SSO error: {detail}")).into());
    }

    let code = query
        .code
        .ok_or_else(|| ConductorError::msg("missing OIDC code"))?;
    let oidc_state = query
        .state
        .ok_or_else(|| ConductorError::msg("missing OIDC state"))?;

    let (code_verifier, nonce) = state
        .take_oidc_pending(&oidc_state)
        .ok_or_else(|| ConductorError::msg("invalid or expired OIDC state"))?;

    let runtime = state
        .db
        .instance()
        .sso_runtime()
        .await?
        .ok_or_else(|| ConductorError::msg("SSO is not enabled"))?;

    let profile = exchange_code(
        &runtime.issuer_url,
        &runtime.client_id,
        &runtime.client_secret,
        &runtime.redirect_uri,
        &code,
        &code_verifier,
        &nonce,
    )
    .await?;

    let user = match state
        .db
        .users()
        .handle_sso_login(
            &profile.issuer,
            &profile.subject,
            &profile.email,
            &profile.display_name,
        )
        .await
    {
        Ok(user) => user,
        Err(SsoLoginError::IdentityConflict) => {
            return Err(ConductorError::Conflict(
                "this email is already linked to a different SSO identity".into(),
            )
            .into())
        }
        Err(SsoLoginError::Database(error)) => return Err(error.into()),
    };

    let frontend = frontend_base(&state).await?;
    let base = frontend.trim_end_matches('/');

    match user.status {
        UserStatus::Active => {
            let session = issue_session(&state, user).await?;
            let redirect = format!(
                "{}/auth/callback#token={}&expires_at={}",
                base,
                urlencoding_encode(&session.token),
                urlencoding_encode(&session.expires_at.to_rfc3339()),
            );
            Ok(Redirect::temporary(&redirect))
        }
        UserStatus::Pending => {
            let redirect = format!("{base}/pending");
            Ok(Redirect::temporary(&redirect))
        }
        UserStatus::Disabled => {
            let redirect = format!("{}/login?error=disabled", base);
            Ok(Redirect::temporary(&redirect))
        }
        UserStatus::Invited => {
            // handle_sso_login should have activated invited users
            let session = issue_session(&state, user).await?;
            let redirect = format!(
                "{}/auth/callback#token={}&expires_at={}",
                base,
                urlencoding_encode(&session.token),
                urlencoding_encode(&session.expires_at.to_rfc3339()),
            );
            Ok(Redirect::temporary(&redirect))
        }
    }
}

async fn issue_session(state: &AppState, user: User) -> ApiResult<AuthSession> {
    let jwt = state.jwt().await.ok_or(ConductorError::SetupRequired)?;
    let session_version = state
        .db
        .users()
        .session_version(user.id)
        .await?
        .ok_or(ConductorError::Unauthorized)?;
    let (token, exp) = jwt.issue(user.id, &user.email, user.primary_role, session_version)?;
    let expires_at = Utc.timestamp_opt(exp, 0).single().unwrap_or_else(Utc::now);

    Ok(AuthSession {
        token,
        user,
        expires_at,
    })
}

async fn frontend_base(state: &AppState) -> ApiResult<String> {
    if let Some(instance) = state.db.instance().get().await? {
        if let Some(url) = instance.public_url.filter(|u| !u.is_empty()) {
            return Ok(url);
        }
    }
    Ok(std::env::var("CONDUCTOR_PUBLIC_URL").unwrap_or_else(|_| "http://127.0.0.1:5174".into()))
}

fn urlencoding_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
