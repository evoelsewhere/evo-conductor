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

fn can_access_member_secrets(actor: &conductor_domain::User, member_id: Uuid) -> bool {
    actor.id == member_id || actor.primary_role.can_manage_members()
}

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

pub async fn list_for_member(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(member_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ConnectionSecret>>> {
    if !can_access_member_secrets(&actor, member_id) {
        return Err(ConductorError::Forbidden.into());
    }
    state
        .db
        .users()
        .find_by_id(member_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    Ok(Json(state.db.secrets().list_for_user(member_id).await?))
}

pub async fn create_for_member(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(member_id): Path<Uuid>,
    Json(req): Json<CreateSecretRequest>,
) -> ApiResult<Json<CreatedSecret>> {
    if !can_access_member_secrets(&actor, member_id) {
        return Err(ConductorError::Forbidden.into());
    }
    state
        .db
        .users()
        .find_by_id(member_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
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
    let (token, prefix, hash) = generate_connection_token();
    let secret = state
        .db
        .secrets()
        .insert(
            member_id,
            req.name.trim(),
            &prefix,
            &hash,
            &req.scopes,
            req.expires_at,
        )
        .await?;
    Ok(Json(CreatedSecret { secret, token }))
}

pub async fn revoke_for_member(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path((member_id, secret_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    if !can_access_member_secrets(&actor, member_id) {
        return Err(ConductorError::Forbidden.into());
    }
    if !state.db.secrets().revoke(secret_id, member_id).await? {
        return Err(ConductorError::NotFound("secret".into()).into());
    }
    state
        .realtime
        .disconnect_secret(secret_id, "secret_revoked");
    Ok(Json(serde_json::json!({ "revoked": true })))
}
