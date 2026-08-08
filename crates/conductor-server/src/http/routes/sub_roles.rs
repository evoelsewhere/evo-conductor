use axum::{extract::State, Json};
use conductor_domain::{ConductorError, CreateSubRoleRequest, SubRole};

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<Vec<SubRole>>> {
    Ok(Json(state.db.roles().list_sub_roles().await?))
}

pub async fn create(
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
