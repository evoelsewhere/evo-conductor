use axum::{extract::State, http::HeaderMap, Json};
use conductor_auth::hash_token;
use conductor_domain::core::constants::auth::AUTH_SCHEME_BEARER;
use conductor_domain::core::constants::token::CONNECTION_TOKEN_PREFIX;
use conductor_domain::{ConductorError, ManagedResource, SecretScope, UserStatus};

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Vec<ManagedResource>>> {
    Ok(Json(state.db.resources().list_visible_to(user.id).await?))
}

/// EvoFlux subscribe endpoint — `Authorization: Bearer evc_...`.
pub async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ManagedResource>>> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix(AUTH_SCHEME_BEARER))
        .ok_or(ConductorError::Unauthorized)?;

    if !auth.starts_with(CONNECTION_TOKEN_PREFIX) {
        return Err(ConductorError::Unauthorized.into());
    }

    let hash = hash_token(auth);
    let secret = state
        .db
        .secrets()
        .find_by_hash(&hash)
        .await?
        .ok_or(ConductorError::Unauthorized)?;

    if let Some(exp) = secret.expires_at {
        if exp < chrono::Utc::now() {
            return Err(ConductorError::Unauthorized.into());
        }
    }

    if !secret.scopes.contains(&SecretScope::SubscribeResources) {
        return Err(ConductorError::Forbidden.into());
    }

    let owner = state
        .db
        .users()
        .find_by_id(secret.owner_user_id)
        .await?
        .ok_or(ConductorError::Unauthorized)?;
    if owner.status != UserStatus::Active {
        return Err(ConductorError::Unauthorized.into());
    }

    state.db.secrets().mark_used(secret.id).await?;

    Ok(Json(
        state
            .db
            .resources()
            .list_visible_to(secret.owner_user_id)
            .await?,
    ))
}
