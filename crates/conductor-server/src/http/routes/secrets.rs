use axum::{
    extract::{Path, State},
    Json,
};
use conductor_auth::generate_connection_token;
use conductor_domain::{
    ConnectionSecret, CreateSecretRequest, CreatedSecret, ConductorError, SecretScope,
};
use uuid::Uuid;

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

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
    if req.name.trim().is_empty() {
        return Err(ConductorError::msg("name is required").into());
    }

    let scopes = if req.scopes.is_empty() {
        vec![
            SecretScope::SubscribeResources,
            SecretScope::ReportTelemetry,
            SecretScope::SyncInventory,
        ]
    } else {
        req.scopes
    };

    let (token, prefix, hash) = generate_connection_token();
    let secret = state
        .db
        .secrets()
        .insert(user.id, &req.name, &prefix, &hash, &scopes, req.expires_at)
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
    Ok(Json(serde_json::json!({ "revoked": true })))
}
