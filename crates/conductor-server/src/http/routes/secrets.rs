use axum::{
    extract::{Path, State},
    Json,
};
use conductor_auth::generate_connection_token;
use conductor_domain::{ConductorError, ConnectionSecret, CreateSecretRequest, CreatedSecret};
use uuid::Uuid;

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Vec<ConnectionSecret>>> {
    Ok(Json(state.db.secrets().list_for_user(user.id).await?))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<CreateSecretRequest>,
) -> ApiResult<Json<CreatedSecret>> {
    if req.name.trim().is_empty() || req.name.trim().len() > 120 {
        return Err(ConductorError::msg("name is required").into());
    }
    if req.scopes.is_empty() {
        return Err(ConductorError::msg("at least one scope is required").into());
    }
    if req
        .expires_at
        .is_some_and(|expires| expires <= chrono::Utc::now())
    {
        return Err(ConductorError::msg("expires_at must be in the future").into());
    }
    let scopes = req.scopes;

    let (token, prefix, hash) = generate_connection_token();
    let secret = state
        .db
        .secrets()
        .insert(
            user.id,
            req.name.trim(),
            &prefix,
            &hash,
            &scopes,
            req.expires_at,
        )
        .await?;

    Ok(Json(CreatedSecret { secret, token }))
}

pub async fn revoke(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let ok = state.db.secrets().revoke(id, user.id).await?;
    if !ok {
        return Err(ConductorError::NotFound("secret".into()).into());
    }
    state.realtime.disconnect_secret(id, "secret_revoked");
    Ok(Json(serde_json::json!({ "revoked": true })))
}
