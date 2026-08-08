use axum::{extract::State, http::HeaderMap, Json};
use conductor_auth::hash_token;
use conductor_domain::{ConductorError, ManagedResource, SecretScope};

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<Vec<ManagedResource>>> {
    Ok(Json(state.db.resources().list().await?))
}

/// EvoFlux subscribe endpoint — `Authorization: Bearer evc_...`.
pub async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ManagedResource>>> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ConductorError::Unauthorized)?;

    if !auth.starts_with("evc_") {
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

    if !secret
        .scopes
        .iter()
        .any(|s| *s == SecretScope::SubscribeResources)
    {
        return Err(ConductorError::Forbidden.into());
    }

    Ok(Json(state.db.resources().list().await?))
}
