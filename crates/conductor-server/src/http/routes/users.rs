use axum::{
    extract::{Path, Query, State},
    Json,
};
use conductor_auth::{generate_temp_password, hash_password};
use conductor_domain::{
    ApproveMemberRequest, ConductorError, CreateMemberRequest, CreatedMember, MemberListQuery,
    MemberListResponse, PrimaryRole, ResetPasswordResponse, UpdateMemberRequest, User, UserStatus,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub role: Option<String>,
    pub tag: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

pub async fn list(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<MemberListResponse>> {
    if !actor.primary_role.can_list_members() {
        return Err(ConductorError::Forbidden.into());
    }

    let status = query.status.as_deref().map(UserStatus::parse);

    let role = query
        .role
        .as_deref()
        .and_then(PrimaryRole::parse);

    let filter = MemberListQuery {
        q: query.q,
        status: if actor.primary_role.can_manage_members() {
            status
        } else {
            Some(UserStatus::Active)
        },
        role,
        tag: query.tag,
        page: query.page.unwrap_or(1),
        limit: query.limit.unwrap_or(50),
        active_only: !actor.primary_role.can_manage_members(),
    };

    let (items, total) = state.db.users().list_filtered(&filter).await?;
    Ok(Json(MemberListResponse {
        items,
        total,
        page: filter.page.max(1),
        limit: filter.limit.clamp(1, 100),
    }))
}

pub async fn pending_count(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    if !actor.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    let count = state.db.users().pending_count().await?;
    Ok(Json(serde_json::json!({ "count": count })))
}

pub async fn get(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    if !actor.primary_role.can_list_members() && actor.id != id {
        return Err(ConductorError::Forbidden.into());
    }
    let user = state
        .db
        .users()
        .find_by_id(id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    if !actor.primary_role.can_manage_members()
        && user.status != UserStatus::Active
        && actor.id != id
    {
        return Err(ConductorError::NotFound("member".into()).into());
    }
    Ok(Json(user))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Json(req): Json<CreateMemberRequest>,
) -> ApiResult<Json<CreatedMember>> {
    if !actor.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    if !req.email.contains('@') {
        return Err(ConductorError::msg("valid email is required").into());
    }
    if req.display_name.trim().is_empty() {
        return Err(ConductorError::msg("display_name is required").into());
    }
    if state
        .db
        .users()
        .find_by_email(&req.email)
        .await?
        .is_some()
    {
        return Err(ConductorError::Conflict("email already registered".into()).into());
    }

    let temp = generate_temp_password();
    let hash = hash_password(&temp)?;
    let user = state
        .db
        .users()
        .create_invited(&req, &hash, actor.id)
        .await?;

    Ok(Json(CreatedMember {
        user,
        temporary_password: temp,
    }))
}

pub async fn update(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMemberRequest>,
) -> ApiResult<Json<User>> {
    if !actor.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    let _ = state
        .db
        .users()
        .find_by_id(id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;

    let user = state
        .db
        .users()
        .update_member(
            id,
            req.display_name.as_deref(),
            req.primary_role,
            req.sub_role_ids.as_deref(),
            req.tag_ids.as_deref(),
        )
        .await?;
    Ok(Json(user))
}

pub async fn approve(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveMemberRequest>,
) -> ApiResult<Json<User>> {
    if !actor.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    let existing = state
        .db
        .users()
        .find_by_id(id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    if existing.status != UserStatus::Pending && existing.status != UserStatus::Invited {
        return Err(ConductorError::msg("member is not pending approval").into());
    }

    let user = state
        .db
        .users()
        .approve(
            id,
            actor.id,
            req.primary_role,
            req.sub_role_ids.as_deref(),
            req.tag_ids.as_deref(),
        )
        .await?;
    Ok(Json(user))
}

pub async fn disable(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    if !actor.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    if actor.id == id {
        return Err(ConductorError::msg("cannot disable yourself").into());
    }
    Ok(Json(
        state
            .db
            .users()
            .set_status(id, UserStatus::Disabled)
            .await?,
    ))
}

pub async fn enable(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    if !actor.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(Json(
        state.db.users().set_status(id, UserStatus::Active).await?,
    ))
}

pub async fn reset_password(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ResetPasswordResponse>> {
    if !actor.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    let _ = state
        .db
        .users()
        .find_by_id(id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;

    let temp = generate_temp_password();
    let hash = hash_password(&temp)?;
    state.db.users().set_password(id, &hash, true).await?;
    Ok(Json(ResetPasswordResponse {
        temporary_password: temp,
    }))
}
