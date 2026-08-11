use axum::{
    extract::{Path, State},
    Json,
};
use conductor_domain::{
    ConductorError, CreateSubRoleRequest, CreateTagRequest, SubRole, Tag, UpdateSubRoleRequest,
    UpdateTagRequest,
};
use serde::{Deserialize, Serialize};

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

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

fn validate_slug(value: &str) -> Result<(), ConductorError> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(ConductorError::msg(
            "slug must use lowercase letters, digits, or hyphens (max 64)",
        ))
    }
}

fn validate_name_and_color(name: &str, color: Option<&str>) -> Result<(), ConductorError> {
    if name.trim().is_empty() || name.trim().len() > 120 {
        return Err(ConductorError::msg("name is required (max 120)"));
    }
    if let Some(color) = color.filter(|value| !value.is_empty()) {
        let valid = color.len() == 7
            && color.starts_with('#')
            && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit());
        if !valid {
            return Err(ConductorError::msg("color must be a 6-digit hex value"));
        }
    }
    Ok(())
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
    validate_slug(&req.slug)?;
    validate_name_and_color(&req.name, req.color.as_deref())?;
    if state.db.roles().sub_role_slug_exists(&req.slug).await? {
        return Err(ConductorError::Conflict("sub-role slug already exists".into()).into());
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
    if let Some(name) = req.name.as_deref() {
        validate_name_and_color(name, req.color.as_deref())?;
    } else {
        validate_name_and_color("unchanged", req.color.as_deref())?;
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
    validate_slug(&req.slug)?;
    validate_name_and_color(&req.name, req.color.as_deref())?;
    if state.db.roles().tag_slug_exists(&req.slug).await? {
        return Err(ConductorError::Conflict("tag slug already exists".into()).into());
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
    if let Some(name) = req.name.as_deref() {
        validate_name_and_color(name, req.color.as_deref())?;
    } else {
        validate_name_and_color("unchanged", req.color.as_deref())?;
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

/// Generic tag assignment for members and governed resources.
pub async fn get_entity_tags(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((entity_type, entity_id)): Path<(String, String)>,
) -> ApiResult<Json<EntityTagsResponse>> {
    if !user.primary_role.can_manage_tags() {
        return Err(ConductorError::Forbidden.into());
    }
    validate_entity_type(&entity_type)?;
    if entity_id.is_empty() || entity_id.len() > 200 {
        return Err(ConductorError::msg("entity_id is required (max 200)").into());
    }
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
    if entity_type == "member" && !user.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    if entity_id.is_empty() || entity_id.len() > 200 {
        return Err(ConductorError::msg("entity_id is required (max 200)").into());
    }
    let unique: std::collections::HashSet<&str> = req.tag_ids.iter().map(String::as_str).collect();
    if unique.len() != req.tag_ids.len() || !state.db.roles().all_tags_exist(&req.tag_ids).await? {
        return Err(ConductorError::msg("tag_ids contains duplicates or unknown tags").into());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_labels_accept_only_safe_slugs_and_colors() {
        assert!(validate_slug("platform-team").is_ok());
        assert!(validate_slug("Platform Team").is_err());
        assert!(validate_name_and_color("Platform", Some("#4c66d6")).is_ok());
        assert!(validate_name_and_color("Platform", Some("url(evil)")).is_err());
    }

    #[test]
    fn generic_entity_type_is_bounded() {
        assert!(validate_entity_type("managed_resource").is_ok());
        assert!(validate_entity_type("Managed-Resource").is_err());
    }
}
