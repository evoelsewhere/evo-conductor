use axum::{
    extract::{Path, State},
    Extension, Json,
};
use conductor_domain::{
    AuthorizationTarget, ConductorError, CreateSubRoleRequest, CreateTagRequest, PrimaryRole,
    SubRole, Tag, TargetType, UpdateSubRoleRequest, UpdateTagRequest,
};
use conductor_storage::repos::TaxonomyDeleteResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::{ApiError, ApiResult};
use crate::core::state::AppState;
use crate::http::authorization::{authorize_current_browser_target, RouteAuthorization};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagEntityType {
    Member,
    Resource,
}

impl TagEntityType {
    fn parse(value: &str) -> Result<Self, ConductorError> {
        match value {
            "member" => Ok(Self::Member),
            "resource" => Ok(Self::Resource),
            _ => Err(ConductorError::msg(
                "entity_type must be either member or resource",
            )),
        }
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

pub async fn list_sub_roles(State(state): State<AppState>) -> ApiResult<Json<Vec<SubRole>>> {
    Ok(Json(state.db.roles().list_sub_roles().await?))
}

pub async fn create_sub_role(
    State(state): State<AppState>,
    Json(req): Json<CreateSubRoleRequest>,
) -> ApiResult<Json<SubRole>> {
    validate_slug(&req.slug)?;
    validate_name_and_color(&req.name, req.color.as_deref())?;
    if state.db.roles().sub_role_slug_exists(&req.slug).await? {
        return Err(ApiError::conflict(
            "sub_role_slug_conflict",
            "sub-role slug already exists",
        ));
    }
    Ok(Json(state.db.roles().create_sub_role(&req).await?))
}

pub async fn update_sub_role(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSubRoleRequest>,
) -> ApiResult<Json<SubRole>> {
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
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    match state.db.roles().delete_sub_role(&id).await? {
        TaxonomyDeleteResult::Deleted => {}
        TaxonomyDeleteResult::NotFound => {
            return Err(ConductorError::NotFound("sub_role".into()).into())
        }
        TaxonomyDeleteResult::Referenced => {
            return Err(ApiError::conflict(
                "sub_role_referenced",
                "sub-role is referenced",
            ))
        }
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn list_tags(State(state): State<AppState>) -> ApiResult<Json<Vec<Tag>>> {
    Ok(Json(state.db.roles().list_tags().await?))
}

pub async fn create_tag(
    State(state): State<AppState>,
    Json(req): Json<CreateTagRequest>,
) -> ApiResult<Json<Tag>> {
    validate_slug(&req.slug)?;
    validate_name_and_color(&req.name, req.color.as_deref())?;
    if state.db.roles().tag_slug_exists(&req.slug).await? {
        return Err(ApiError::conflict(
            "tag_slug_conflict",
            "tag slug already exists",
        ));
    }
    Ok(Json(state.db.roles().create_tag(&req).await?))
}

pub async fn update_tag(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateTagRequest>,
) -> ApiResult<Json<Tag>> {
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
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    match state.db.roles().delete_tag(&id).await? {
        TaxonomyDeleteResult::Deleted => {}
        TaxonomyDeleteResult::NotFound => return Err(ConductorError::NotFound("tag".into()).into()),
        TaxonomyDeleteResult::Referenced => {
            return Err(ApiError::conflict("tag_referenced", "tag is referenced"))
        }
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// Generic tag assignment for members and governed resources.
pub async fn get_entity_tags(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(user): AuthUser,
    Path((entity_type, entity_id)): Path<(String, String)>,
) -> ApiResult<Json<EntityTagsResponse>> {
    let (target, entity_id) =
        authorize_tag_target(&state, &route, &user, &entity_type, &entity_id).await?;
    let tag_ids = state
        .db
        .roles()
        .tag_ids_for_entity(target, &entity_id)
        .await?;
    Ok(Json(EntityTagsResponse {
        entity_type: target.to_string(),
        entity_id,
        tag_ids,
    }))
}

pub async fn set_entity_tags(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(user): AuthUser,
    Path((entity_type, entity_id)): Path<(String, String)>,
    Json(req): Json<SetEntityTagsRequest>,
) -> ApiResult<Json<EntityTagsResponse>> {
    let (target, entity_id) =
        authorize_tag_target(&state, &route, &user, &entity_type, &entity_id).await?;
    let unique: std::collections::HashSet<&str> = req.tag_ids.iter().map(String::as_str).collect();
    if unique.len() != req.tag_ids.len() || !state.db.roles().all_tags_exist(&req.tag_ids).await? {
        return Err(ConductorError::msg("tag_ids contains duplicates or unknown tags").into());
    }
    let tag_ids = state
        .db
        .roles()
        .set_entity_tags(target, &entity_id, &req.tag_ids)
        .await?;
    Ok(Json(EntityTagsResponse {
        entity_type: target.to_string(),
        entity_id,
        tag_ids,
    }))
}

async fn authorize_tag_target<'a>(
    state: &AppState,
    route: &RouteAuthorization,
    user: &conductor_domain::User,
    entity_type: &'a str,
    entity_id: &str,
) -> ApiResult<(&'a str, String)> {
    let target = TagEntityType::parse(entity_type)?;
    let id = Uuid::parse_str(entity_id)
        .map_err(|_| ConductorError::NotFound(entity_type.to_string()))?;
    match target {
        TagEntityType::Member => {
            let member = state
                .db
                .users()
                .find_by_id(id)
                .await?
                .ok_or_else(|| ConductorError::NotFound("member".into()))?;
            let project_id = state
                .db
                .instance()
                .authorization_project_id()
                .await?
                .ok_or(ConductorError::SetupRequired)?;
            authorize_current_browser_target(
                state,
                route,
                user,
                AuthorizationTarget {
                    project_id: Some(project_id),
                    target_type: TargetType::Taxonomy,
                    target_id: Some(member.id),
                    owner_id: None,
                    resource_kind: None,
                    lifecycle: None,
                    effective_audience: None,
                },
            )
            .await?;
        }
        TagEntityType::Resource => {
            let resource = state
                .db
                .resources()
                .find_by_id_for_authorization(id)
                .await?
                .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
            let decision = authorize_current_browser_target(
                state,
                route,
                user,
                AuthorizationTarget {
                    project_id: Some(resource.project_id),
                    target_type: TargetType::Taxonomy,
                    target_id: Some(resource.id),
                    owner_id: resource.owner_user_id,
                    resource_kind: Some(resource.kind),
                    lifecycle: None,
                    effective_audience: None,
                },
            )
            .await;
            if let Err(error) = decision {
                if user.primary_role == PrimaryRole::Contribute
                    && resource.owner_user_id != Some(user.id)
                {
                    return Err(ConductorError::NotFound("resource".into()).into());
                }
                return Err(error);
            }
        }
    }
    Ok((entity_type, id.to_string()))
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
    fn tag_assignment_targets_are_closed() {
        assert!(matches!(
            TagEntityType::parse("member"),
            Ok(TagEntityType::Member)
        ));
        assert!(matches!(
            TagEntityType::parse("resource"),
            Ok(TagEntityType::Resource)
        ));
        assert!(TagEntityType::parse("managed_resource").is_err());
    }
}
