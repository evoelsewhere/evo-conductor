//! The local `jira_tasks` mirror's read surface — the task-level usage
//! report's task list/search/detail (browser only) — plus the one
//! connection route in this file: `record_activation`, which the
//! jira-task-assistant plugin calls directly with its own connection token
//! whenever the agent starts work on an issue. That's the entire mechanism
//! behind precise per-task attribution, and it needs no evoflux core
//! involvement at all — the plugin talks to Conductor exactly as it already
//! talks to Jira.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use conductor_domain::{
    AuthorizationTarget, ConductorError, JiraTask, JiraTaskListResponse, TargetType,
    TaskActivationRequest,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{ApiError, ApiResult, AppState};
use crate::http::authorization::{authorize_current_connection_target, RouteAuthorization};
use crate::http::extractors::ConnectionPrincipal;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<JiraTaskListResponse>> {
    let project_id = project_id(&state).await?;
    let tasks = match query.q.as_deref().filter(|q| !q.trim().is_empty()) {
        Some(q) => state.db.jira_tasks().search(project_id, q).await?,
        None => state.db.jira_tasks().list(project_id).await?,
    };
    Ok(Json(JiraTaskListResponse { tasks }))
}

pub async fn detail(
    State(state): State<AppState>,
    Path(issue_key): Path<String>,
) -> ApiResult<Json<JiraTask>> {
    let task = state
        .db
        .jira_tasks()
        .find(project_id(&state).await?, &issue_key)
        .await?
        .ok_or_else(|| ApiError::from(ConductorError::NotFound(issue_key)))?;
    Ok(Json(task))
}

#[derive(Debug, Serialize)]
pub struct TaskActivationResponse {
    pub recorded: bool,
}

/// The plugin's own activation call: "the calling member just started
/// `issue_key`". `principal.user.id` — the connection token's owner, not
/// anything the request body claims — is who the activation is recorded
/// for, so one member's plugin can never backdate or attribute usage to
/// another member's task window.
pub async fn record_activation(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    Extension(principal): Extension<ConnectionPrincipal>,
    Json(request): Json<TaskActivationRequest>,
) -> ApiResult<Json<TaskActivationResponse>> {
    let project_id = project_id(&state).await?;
    authorize_current_connection_target(
        &state,
        &route,
        &principal,
        AuthorizationTarget {
            project_id: Some(project_id),
            target_type: TargetType::Project,
            target_id: None,
            owner_id: Some(principal.user.id),
            resource_kind: None,
            lifecycle: None,
            effective_audience: None,
        },
    )
    .await?;
    let issue_key = request.issue_key.trim();
    if issue_key.is_empty() {
        return Err(ConductorError::msg("issue_key is required").into());
    }
    state
        .db
        .task_activations()
        .record(project_id, principal.user.id, issue_key)
        .await?;
    Ok(Json(TaskActivationResponse { recorded: true }))
}

async fn project_id(state: &AppState) -> Result<Uuid, ApiError> {
    state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or_else(|| ApiError::from(ConductorError::SetupRequired))
}
