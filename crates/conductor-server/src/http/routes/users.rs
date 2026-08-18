use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use conductor_auth::{generate_temp_password, hash_password_async};
use conductor_domain::{
    role_has_permission, ApproveMemberRequest, AuthorizationTarget, ConductorError,
    CreateMemberRequest, CreatedMember, MemberListQuery, PermissionKey, PrimaryRole,
    ResetPasswordResponse, TargetType, UpdateMemberRequest, User, UserStatus,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use conductor_storage::{
    ApproveMemberAccess, ChangeMemberStatus, MemberAccessChange, MemberAccessError,
    UpdateAccessProfile,
};

use crate::core::error::{ApiError, ApiResult};
use crate::core::state::AppState;
use crate::http::authorization::{authorize_current_browser_target, RouteAuthorization};
use crate::http::extractors::AuthUser;

fn member_access_error(error: MemberAccessError) -> ApiError {
    match error {
        MemberAccessError::Database(error) => error.into(),
        MemberAccessError::TargetNotFound => ApiError::with_public_code(
            ConductorError::NotFound("member".into()),
            "member_not_found",
        ),
        MemberAccessError::ActorNotFound | MemberAccessError::ActorNotAuthorized => {
            ConductorError::Forbidden.into()
        }
        MemberAccessError::SelfPrimaryRoleChange => {
            ApiError::conflict("self_role_change", error.to_string())
        }
        MemberAccessError::SelfDisable => ApiError::conflict("self_disable", error.to_string()),
        MemberAccessError::LastActiveAdmin => {
            ApiError::conflict("last_active_admin", error.to_string())
        }
        MemberAccessError::EmptyDisplayName
        | MemberAccessError::DuplicateSubRoleId
        | MemberAccessError::DuplicateTagId
        | MemberAccessError::UnknownSubRoleId
        | MemberAccessError::UnknownTagId
        | MemberAccessError::UnsupportedStatus
        | MemberAccessError::NotPendingApproval => ConductorError::msg(error.to_string()).into(),
        MemberAccessError::InvalidPersistedPrincipal(_)
        | MemberAccessError::InvalidPersistedCredential(_) => ConductorError::Unauthorized.into(),
        MemberAccessError::ProjectNotConfigured => ConductorError::SetupRequired.into(),
    }
}

fn publish_member_access_change(
    state: &AppState,
    route: &RouteAuthorization,
    change: &MemberAccessChange,
) {
    state.authorization.observe_member_access_change(
        route.request_context(),
        route.route_spec().action,
        change,
    );
    tracing::info!(
        actor_id = %change.actor_id,
        target_id = %change.target_id,
        before_role = change.before.primary_role.as_str(),
        after_role = change.after.primary_role.as_str(),
        before_status = change.before.status.as_str(),
        after_status = change.after.status.as_str(),
        before_sub_role_count = change.before.sub_role_ids.len(),
        after_sub_role_count = change.after.sub_role_ids.len(),
        before_tag_count = change.before.tag_ids.len(),
        after_tag_count = change.after.tag_ids.len(),
        admin_elevation = change.admin_elevation,
        audience_changed = change.audience_changed,
        revoked_credential_count = change.revoked_credentials.len(),
        status_reason = change.status_reason.map(|reason| reason.as_str()),
        "member access change committed"
    );

    for credential in &change.revoked_credentials {
        state
            .realtime
            .disconnect_secret(credential.credential_id, credential.reason);
    }
    if change.audience_changed && change.after.status != UserStatus::Disabled {
        state.realtime.resync_owner_resources(change.target_id);
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub role: Option<String>,
    pub tag: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct DirectoryMember {
    id: Uuid,
    display_name: String,
    primary_role: PrimaryRole,
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
) -> ApiResult<Json<serde_json::Value>> {
    let status = query
        .status
        .as_deref()
        .map(|value| {
            UserStatus::parse(value)
                .ok_or_else(|| ConductorError::msg("status must be a supported member status"))
        })
        .transpose()?;

    let role = query
        .role
        .as_deref()
        .map(|value| {
            PrimaryRole::parse(value)
                .ok_or_else(|| ConductorError::msg("role must be admin, contribute, or user"))
        })
        .transpose()?;

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    if role_has_permission(actor.primary_role, PermissionKey::MemberManage) {
        let filter = MemberListQuery {
            q: query.q,
            status,
            role,
            tag: query.tag,
            page,
            limit,
            active_only: false,
        };
        let (items, total) = state.db.users().list_filtered(&filter).await?;
        return Ok(Json(serde_json::json!({
            "items": items,
            "total": total,
            "page": page,
            "limit": limit,
        })));
    }

    let (items, total) = state
        .db
        .users()
        .list_active_directory(query.q.as_deref(), role, page, limit)
        .await?;
    let items: Vec<DirectoryMember> = items
        .into_iter()
        .map(|member| DirectoryMember {
            id: member.id,
            display_name: member.display_name,
            primary_role: member.primary_role,
        })
        .collect();
    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit,
    })))
}

pub async fn pending_count(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let count = state.db.users().pending_count().await?;
    Ok(Json(serde_json::json!({ "count": count })))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    let user = authorize_member_target(&state, &route, &actor, id).await?;
    Ok(Json(user))
}

async fn authorize_member_target(
    state: &AppState,
    route: &RouteAuthorization,
    actor: &conductor_domain::User,
    member_id: Uuid,
) -> ApiResult<User> {
    let user = state
        .db
        .users()
        .find_by_id(member_id)
        .await?
        .ok_or_else(|| {
            ApiError::with_public_code(
                ConductorError::NotFound("member".into()),
                "member_not_found",
            )
        })?;
    let project_id = state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or(ConductorError::SetupRequired)?;
    authorize_current_browser_target(
        state,
        route,
        actor,
        AuthorizationTarget {
            project_id: Some(project_id),
            target_type: TargetType::Member,
            target_id: Some(user.id),
            owner_id: Some(user.id),
            resource_kind: None,
            lifecycle: None,
            effective_audience: None,
        },
    )
    .await?;
    Ok(user)
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Json(req): Json<CreateMemberRequest>,
) -> ApiResult<Json<CreatedMember>> {
    if !req.email.contains('@') {
        return Err(ConductorError::msg("valid email is required").into());
    }
    if req.display_name.trim().is_empty() {
        return Err(ConductorError::msg("display_name is required").into());
    }
    if state.db.users().find_by_email(&req.email).await?.is_some() {
        return Err(ApiError::conflict(
            "email_already_registered",
            "email already registered",
        ));
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
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMemberRequest>,
) -> ApiResult<Json<User>> {
    if req
        .display_name
        .as_ref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(ConductorError::msg("display_name cannot be empty").into());
    }
    authorize_member_target(&state, &route, &actor, id).await?;
    let result = state
        .db
        .member_access()
        .update_access_profile(UpdateAccessProfile {
            actor_id: actor.id,
            target_id: id,
            display_name: req.display_name.map(|name| name.trim().to_string()),
            primary_role: req.primary_role,
            sub_role_ids: req.sub_role_ids,
            tag_ids: req.tag_ids,
        })
        .await
        .map_err(member_access_error)?;
    publish_member_access_change(&state, &route, &result.change);
    Ok(Json(result.user))
}

pub async fn approve(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveMemberRequest>,
) -> ApiResult<Json<User>> {
    authorize_member_target(&state, &route, &actor, id).await?;
    let result = state
        .db
        .member_access()
        .approve_member(ApproveMemberAccess {
            actor_id: actor.id,
            target_id: id,
            primary_role: req.primary_role,
            sub_role_ids: req.sub_role_ids,
            tag_ids: req.tag_ids,
        })
        .await
        .map_err(member_access_error)?;
    publish_member_access_change(&state, &route, &result.change);
    Ok(Json(result.user))
}

pub async fn disable(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    authorize_member_target(&state, &route, &actor, id).await?;
    let result = state
        .db
        .member_access()
        .set_member_status(ChangeMemberStatus::disable(actor.id, id))
        .await
        .map_err(member_access_error)?;
    if result.change.before.status != UserStatus::Disabled {
        state.realtime.disconnect_owner(id, "owner_disabled");
    }
    publish_member_access_change(&state, &route, &result.change);
    Ok(Json(result.user))
}

pub async fn enable(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    authorize_member_target(&state, &route, &actor, id).await?;
    let result = state
        .db
        .member_access()
        .set_member_status(ChangeMemberStatus::enable(actor.id, id))
        .await
        .map_err(member_access_error)?;
    publish_member_access_change(&state, &route, &result.change);
    Ok(Json(result.user))
}

pub async fn reset_password(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ResetPasswordResponse>> {
    authorize_member_target(&state, &route, &actor, id).await?;

    let temp = generate_temp_password();
    let hash = hash_password_async(temp.clone()).await?;
    state.db.users().set_password(id, &hash, true).await?;
    Ok(Json(ResetPasswordResponse {
        temporary_password: temp,
    }))
}
