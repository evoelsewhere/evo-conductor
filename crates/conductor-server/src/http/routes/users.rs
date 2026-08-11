use axum::{
    extract::{Path, Query, State},
    Json,
};
use conductor_auth::{generate_temp_password, hash_password_async};
use conductor_domain::{
    ApproveMemberRequest, ConductorError, CreateMemberRequest, CreatedMember, MemberListQuery,
    MemberListResponse, PrimaryRole, ResetPasswordResponse, UpdateMemberRequest, User, UserStatus,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub role: Option<String>,
    pub tag: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

async fn validate_access_ids(
    state: &AppState,
    sub_role_ids: &[String],
    tag_ids: &[String],
) -> ApiResult<()> {
    let unique_sub_roles: std::collections::HashSet<&str> =
        sub_role_ids.iter().map(String::as_str).collect();
    let unique_tags: std::collections::HashSet<&str> = tag_ids.iter().map(String::as_str).collect();
    if unique_sub_roles.len() != sub_role_ids.len()
        || !state.db.roles().all_sub_roles_exist(sub_role_ids).await?
    {
        return Err(
            ConductorError::msg("sub_role_ids contains duplicates or unknown roles").into(),
        );
    }
    if unique_tags.len() != tag_ids.len() || !state.db.roles().all_tags_exist(tag_ids).await? {
        return Err(ConductorError::msg("tag_ids contains duplicates or unknown tags").into());
    }
    Ok(())
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

    let role = query.role.as_deref().and_then(PrimaryRole::parse);

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
    if state.db.users().find_by_email(&req.email).await?.is_some() {
        return Err(ConductorError::Conflict("email already registered".into()).into());
    }
    validate_access_ids(&state, &req.sub_role_ids, &req.tag_ids).await?;

    let temp = generate_temp_password();
    let hash = hash_password_async(temp.clone()).await?;
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
    let existing = state
        .db
        .users()
        .find_by_id(id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;

    if req
        .display_name
        .as_ref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(ConductorError::msg("display_name cannot be empty").into());
    }
    if req
        .primary_role
        .is_some_and(|role| role != existing.primary_role)
    {
        if actor.id == id {
            return Err(
                ConductorError::Conflict("you cannot change your own primary role".into()).into(),
            );
        }
        if existing.primary_role == PrimaryRole::Admin
            && state.db.users().active_admin_count().await? <= 1
        {
            return Err(ConductorError::Conflict(
                "the project must keep at least one active admin".into(),
            )
            .into());
        }
    }
    validate_access_ids(
        &state,
        req.sub_role_ids.as_deref().unwrap_or_default(),
        req.tag_ids.as_deref().unwrap_or_default(),
    )
    .await?;

    let user = state
        .db
        .users()
        .update_member(
            id,
            req.display_name.as_deref().map(str::trim),
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
    validate_access_ids(
        &state,
        req.sub_role_ids.as_deref().unwrap_or_default(),
        req.tag_ids.as_deref().unwrap_or_default(),
    )
    .await?;

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
    let existing = state
        .db
        .users()
        .find_by_id(id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    if existing.primary_role == PrimaryRole::Admin
        && existing.status == UserStatus::Active
        && state.db.users().active_admin_count().await? <= 1
    {
        return Err(ConductorError::Conflict(
            "the project must keep at least one active admin".into(),
        )
        .into());
    }
    let user = state
        .db
        .users()
        .set_status(id, UserStatus::Disabled)
        .await?;
    state.realtime.disconnect_owner(id, "owner_disabled");
    Ok(Json(user))
}

pub async fn enable(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
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
    let hash = hash_password_async(temp.clone()).await?;
    state.db.users().set_password(id, &hash, true).await?;
    Ok(Json(ResetPasswordResponse {
        temporary_password: temp,
    }))
}
