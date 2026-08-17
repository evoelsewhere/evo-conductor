use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use conductor_domain::{
    role_has_permission, AnalyticsView, AnalyticsViewVisibility, AuthorizationTarget,
    ConductorError, CreateAnalyticsViewRequest, PermissionKey, PrimaryRole, TargetType,
    UpdateAnalyticsViewRequest,
};
use conductor_storage::repos::AnalyticsViewWriteError;
use serde::Deserialize;
use uuid::Uuid;

use crate::core::error::{ApiError, ApiResult};
use crate::core::state::AppState;
use crate::http::authorization::{authorize_current_browser_target, RouteAuthorization};
use crate::http::extractors::AuthUser;

pub async fn list(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
) -> ApiResult<Json<Vec<AnalyticsView>>> {
    let project_id = project_id(&state).await?;
    authorize_current_browser_target(
        &state,
        &route,
        &actor,
        AuthorizationTarget {
            project_id: Some(project_id),
            target_type: TargetType::AnalyticsView,
            target_id: None,
            owner_id: None,
            resource_kind: None,
            lifecycle: None,
            // `list_accessible` is the authoritative visibility-filtered
            // collection query for this route.
            effective_audience: Some(true),
        },
    )
    .await?;
    let views = state
        .db
        .analytics_views()
        .list_accessible(
            project_id,
            actor.id,
            actor.primary_role == PrimaryRole::Admin,
        )
        .await?
        .into_iter()
        .filter(|view| can_read_definition(&actor, view))
        .collect();
    Ok(Json(views))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(view_id): Path<Uuid>,
) -> ApiResult<Json<AnalyticsView>> {
    let project_id = project_id(&state).await?;
    let view = load_view_for_policy(&state, project_id, view_id, actor.id).await?;
    authorize_view_target(&state, &route, &actor, &view).await?;
    Ok(Json(view))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Json(mut request): Json<CreateAnalyticsViewRequest>,
) -> ApiResult<Json<AnalyticsView>> {
    normalize_metadata(&mut request.name, &mut request.description);
    let project_id = project_id(&state).await?;
    let view = state
        .db
        .analytics_views()
        .create(project_id, actor.id, &request)
        .await
        .map_err(map_write_error)?;
    Ok(Json(view))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(view_id): Path<Uuid>,
    Json(mut request): Json<UpdateAnalyticsViewRequest>,
) -> ApiResult<Json<AnalyticsView>> {
    normalize_metadata(&mut request.name, &mut request.description);
    let project_id = project_id(&state).await?;
    let current = load_view_for_policy(&state, project_id, view_id, actor.id).await?;
    authorize_view_target(&state, &route, &actor, &current).await?;
    let view = state
        .db
        .analytics_views()
        .update(
            project_id,
            view_id,
            actor.id,
            actor.primary_role == PrimaryRole::Admin,
            &request,
        )
        .await
        .map_err(map_write_error)?;
    Ok(Json(view))
}

#[derive(Debug, Deserialize)]
pub struct DeleteQuery {
    pub revision: u64,
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(view_id): Path<Uuid>,
    Query(query): Query<DeleteQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = project_id(&state).await?;
    let current = load_view_for_policy(&state, project_id, view_id, actor.id).await?;
    authorize_view_target(&state, &route, &actor, &current).await?;
    state
        .db
        .analytics_views()
        .delete(
            project_id,
            view_id,
            actor.id,
            actor.primary_role == PrimaryRole::Admin,
            query.revision,
        )
        .await
        .map_err(map_write_error)?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn project_id(state: &AppState) -> ApiResult<Uuid> {
    state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or_else(|| ConductorError::SetupRequired.into())
}

async fn load_view_for_policy(
    state: &AppState,
    project_id: Uuid,
    view_id: Uuid,
    actor_id: Uuid,
) -> ApiResult<AnalyticsView> {
    // The policy layer, not this loader, decides whether the current actor may
    // use the view. `include_all_private` prevents a visibility-filtered query
    // from turning ownership into an implicit authorization decision.
    state
        .db
        .analytics_views()
        .find_accessible(project_id, view_id, actor_id, true)
        .await?
        .ok_or_else(|| ConductorError::NotFound("analytics view".into()).into())
}

async fn authorize_view_target(
    state: &AppState,
    route: &RouteAuthorization,
    actor: &conductor_domain::User,
    view: &AnalyticsView,
) -> ApiResult<()> {
    authorize_current_browser_target(
        state,
        route,
        actor,
        AuthorizationTarget {
            project_id: Some(view.project_id),
            target_type: TargetType::AnalyticsView,
            target_id: Some(view.id),
            owner_id: Some(view.owner_user_id),
            resource_kind: None,
            lifecycle: None,
            effective_audience: Some(
                (actor.primary_role == PrimaryRole::Admin
                    || view.owner_user_id == actor.id
                    || view.visibility == AnalyticsViewVisibility::Shared)
                    && can_read_definition(actor, view),
            ),
        },
    )
    .await?;
    Ok(())
}

/// A shared view is only readable when its definition is safe for the current
/// reader's telemetry projection. Member and installation selectors are direct
/// identifiers, so a non-owner without cross-member telemetry permission must
/// not receive them merely because the view was marked shared.
fn can_read_definition(actor: &conductor_domain::User, view: &AnalyticsView) -> bool {
    view.owner_user_id == actor.id
        || role_has_permission(actor.primary_role, PermissionKey::TelemetryMemberReadAny)
        || (view.definition.query.member_id.is_none()
            && view.definition.query.installation_id.is_none())
}

fn normalize_metadata(name: &mut String, description: &mut Option<String>) {
    *name = name.trim().to_string();
    *description = description
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
}

fn map_write_error(error: AnalyticsViewWriteError) -> ApiError {
    match error {
        AnalyticsViewWriteError::NotFound => {
            ConductorError::NotFound("analytics view".into()).into()
        }
        AnalyticsViewWriteError::Forbidden => ConductorError::Forbidden.into(),
        AnalyticsViewWriteError::RevisionConflict { current_revision } => ApiError::conflict(
            "revision_conflict",
            format!("analytics view revision mismatch; current revision is {current_revision}"),
        ),
        AnalyticsViewWriteError::NameConflict => ApiError::conflict(
            "analytics_view_name_conflict",
            "analytics view name already exists",
        ),
        AnalyticsViewWriteError::Validation(message) => ConductorError::msg(message).into(),
        AnalyticsViewWriteError::Database(error) => ApiError::from(error),
    }
}
