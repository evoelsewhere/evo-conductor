use axum::{
    extract::{Path, Query, State},
    Json,
};
use conductor_domain::{
    AnalyticsView, ConductorError, CreateAnalyticsViewRequest, PrimaryRole,
    UpdateAnalyticsViewRequest,
};
use conductor_storage::repos::AnalyticsViewWriteError;
use serde::Deserialize;
use uuid::Uuid;

use crate::core::error::{ApiError, ApiResult};
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
) -> ApiResult<Json<Vec<AnalyticsView>>> {
    require_analytics_access(actor.primary_role)?;
    let project_id = project_id(&state).await?;
    Ok(Json(
        state
            .db
            .analytics_views()
            .list_accessible(
                project_id,
                actor.id,
                actor.primary_role == PrimaryRole::Admin,
            )
            .await?,
    ))
}

pub async fn get(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(view_id): Path<Uuid>,
) -> ApiResult<Json<AnalyticsView>> {
    require_analytics_access(actor.primary_role)?;
    let project_id = project_id(&state).await?;
    Ok(Json(
        state
            .db
            .analytics_views()
            .find_accessible(
                project_id,
                view_id,
                actor.id,
                actor.primary_role == PrimaryRole::Admin,
            )
            .await?
            .ok_or_else(|| ConductorError::NotFound("analytics view".into()))?,
    ))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Json(mut request): Json<CreateAnalyticsViewRequest>,
) -> ApiResult<Json<AnalyticsView>> {
    require_analytics_access(actor.primary_role)?;
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
    AuthUser(actor): AuthUser,
    Path(view_id): Path<Uuid>,
    Json(mut request): Json<UpdateAnalyticsViewRequest>,
) -> ApiResult<Json<AnalyticsView>> {
    require_analytics_access(actor.primary_role)?;
    normalize_metadata(&mut request.name, &mut request.description);
    let project_id = project_id(&state).await?;
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
    AuthUser(actor): AuthUser,
    Path(view_id): Path<Uuid>,
    Query(query): Query<DeleteQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_analytics_access(actor.primary_role)?;
    let project_id = project_id(&state).await?;
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

fn require_analytics_access(role: PrimaryRole) -> ApiResult<()> {
    if !role.can_view_telemetry() {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(())
}

async fn project_id(state: &AppState) -> ApiResult<Uuid> {
    state
        .db
        .instance()
        .project_id()
        .await?
        .ok_or_else(|| ConductorError::SetupRequired.into())
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
        AnalyticsViewWriteError::RevisionConflict { current_revision } => ConductorError::Conflict(
            format!("analytics view revision mismatch; current revision is {current_revision}"),
        )
        .into(),
        AnalyticsViewWriteError::NameConflict => {
            ConductorError::Conflict("analytics view name already exists".into()).into()
        }
        AnalyticsViewWriteError::Validation(message) => ConductorError::msg(message).into(),
        AnalyticsViewWriteError::Database(error) => ApiError::from(error),
    }
}
