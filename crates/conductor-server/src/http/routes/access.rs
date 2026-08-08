use axum::{
    extract::{Path, State},
    Json,
};
use conductor_domain::{
    ConductorError, CreateSubRoleRequest, CreateTagRequest, SubRole, Tag, UpdateSubRoleRequest,
    UpdateTagRequest,
};
use serde::{Deserialize, Serialize};

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SetEntityTagsRequest {
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct EntityTagsResponse {
    pub entity_type: String,
    pub entity_id: String,
    pub tag_ids: Vec<String>,
}

fn validate_entity_type(value: &str) -> Result<(), ConductorError> {
    let valid = !value.is_empty()
        && value.len() <= 40
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(ConductorError::msg(
            "entity_type must be lowercase letters, digits, or underscores",
        ))
    }
}

pub async fn list_sub_roles(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Vec<SubRole>>> {
    if !user.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(Json(state.db.roles().list_sub_roles().await?))
}

pub async fn create_sub_role(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<CreateSubRoleRequest>,
) -> ApiResult<Json<SubRole>> {
    if !user.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    if req.slug.trim().is_empty() || req.name.trim().is_empty() {
        return Err(ConductorError::msg("slug and name are required").into());
    }
    Ok(Json(state.db.roles().create_sub_role(&req).await?))
}

pub async fn update_sub_role(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateSubRoleRequest>,
) -> ApiResult<Json<SubRole>> {
    if !user.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    state
        .db
        .roles()
        .update_sub_role(&id, &req)
        .await?
        .ok_or_else(|| ConductorError::NotFound("sub_role".into()).into())
        .map(Json)
}

pub async fn delete_sub_role(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    if !user.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    let deleted = state.db.roles().delete_sub_role(&id).await?;
    if !deleted {
        return Err(ConductorError::NotFound("sub_role".into()).into());
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn list_tags(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Vec<Tag>>> {
    if !user.primary_role.can_manage_tags() {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(Json(state.db.roles().list_tags().await?))
}

pub async fn create_tag(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<CreateTagRequest>,
) -> ApiResult<Json<Tag>> {
    if !user.primary_role.can_manage_tags() {
        return Err(ConductorError::Forbidden.into());
    }
    if req.slug.trim().is_empty() || req.name.trim().is_empty() {
        return Err(ConductorError::msg("slug and name are required").into());
    }
    Ok(Json(state.db.roles().create_tag(&req).await?))
}

pub async fn update_tag(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateTagRequest>,
) -> ApiResult<Json<Tag>> {
    if !user.primary_role.can_manage_tags() {
        return Err(ConductorError::Forbidden.into());
    }
    state
        .db
        .roles()
        .update_tag(&id, &req)
        .await?
        .ok_or_else(|| ConductorError::NotFound("tag".into()).into())
        .map(Json)
}

pub async fn delete_tag(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    if !user.primary_role.can_manage_tags() {
        return Err(ConductorError::Forbidden.into());
    }
    let deleted = state.db.roles().delete_tag(&id).await?;
    if !deleted {
        return Err(ConductorError::NotFound("tag".into()).into());
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// Generic tag assignment: works for member, resource, agent, skill, mcp, etc.
pub async fn get_entity_tags(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((entity_type, entity_id)): Path<(String, String)>,
) -> ApiResult<Json<EntityTagsResponse>> {
    if !user.primary_role.can_manage_tags() {
        return Err(ConductorError::Forbidden.into());
    }
    validate_entity_type(&entity_type)?;
    let tag_ids = state
        .db
        .roles()
        .tag_ids_for_entity(&entity_type, &entity_id)
        .await?;
    Ok(Json(EntityTagsResponse {
        entity_type,
        entity_id,
        tag_ids,
    }))
}

pub async fn set_entity_tags(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((entity_type, entity_id)): Path<(String, String)>,
    Json(req): Json<SetEntityTagsRequest>,
) -> ApiResult<Json<EntityTagsResponse>> {
    if !user.primary_role.can_manage_tags() {
        return Err(ConductorError::Forbidden.into());
    }
    validate_entity_type(&entity_type)?;
    let tag_ids = state
        .db
        .roles()
        .set_entity_tags(&entity_type, &entity_id, &req.tag_ids)
        .await?;
    Ok(Json(EntityTagsResponse {
        entity_type,
        entity_id,
        tag_ids,
    }))
}
