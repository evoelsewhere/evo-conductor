//! Per-project AI provider/tool allow-list policy — the HTTP face of
//! [`conductor_storage::repos::AiPolicyRepo`]. Mirrors `pricing.rs`'s
//! spend-limit handlers exactly: same shape, same authorization model, one
//! table row per (scope, subject) instead of a dollar ceiling.

use axum::extract::{Query, State};
use axum::{Extension, Json};
use conductor_domain::{
    AiPolicyListResponse, AiPolicyScope, AiPolicySelector, AiPolicyView, AuthorizationTarget,
    ConductorError, PrimaryRole, TargetType, UpsertAiPolicyRequest,
};
use conductor_storage::repos::{AiPolicy, UpsertAiPolicy};
use serde::Serialize;
use uuid::Uuid;

use crate::core::{ApiError, ApiResult, AppState};
use crate::http::authorization::{authorize_current_connection_target, RouteAuthorization};
use crate::http::extractors::ConnectionPrincipal;

pub async fn list_ai_policies(
    State(state): State<AppState>,
) -> ApiResult<Json<AiPolicyListResponse>> {
    let response = policy_list_response(&state, project_id(&state).await?).await?;
    Ok(Json(response))
}

/// Connection-authenticated variant: evoflux's periodic sync (Phase 1, T1.2)
/// reads this with its existing connection token, no browser session. The
/// selector (`Selector::None`) has nothing to resolve from the request
/// itself, so — like `client::register`/`client::heartbeat` — the target
/// decision has to be recorded explicitly here, or the boundary middleware
/// fails the request closed for completing without one.
pub async fn client_read_ai_policy(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    Extension(principal): Extension<ConnectionPrincipal>,
) -> ApiResult<Json<AiPolicyListResponse>> {
    let project_id = project_id(&state).await?;
    authorize_current_connection_target(
        &state,
        &route,
        &principal,
        AuthorizationTarget {
            project_id: Some(project_id),
            target_type: TargetType::Project,
            target_id: None,
            owner_id: None,
            resource_kind: None,
            lifecycle: None,
            effective_audience: None,
        },
    )
    .await?;
    let response = policy_list_response(&state, project_id).await?;
    Ok(Json(response))
}

async fn policy_list_response(
    state: &AppState,
    project_id: Uuid,
) -> ApiResult<AiPolicyListResponse> {
    let policies = state
        .db
        .ai_policies()
        .list(project_id)
        .await?
        .into_iter()
        .map(to_view)
        .collect();
    Ok(AiPolicyListResponse { policies })
}

pub async fn upsert_ai_policy(
    State(state): State<AppState>,
    Json(request): Json<UpsertAiPolicyRequest>,
) -> ApiResult<Json<AiPolicyView>> {
    let subject_id = resolve_subject(request.scope, request.subject_id.as_deref())?;
    let stored = state
        .db
        .ai_policies()
        .upsert(
            project_id(&state).await?,
            &UpsertAiPolicy {
                scope: request.scope,
                subject_id,
                default_provider: request.default_provider,
                default_model: request.default_model,
                allowed_providers: request.allowed_providers,
                allowed_tools: request.allowed_tools,
            },
        )
        .await?;
    Ok(Json(to_view(stored)))
}

#[derive(Serialize)]
pub struct DeletedAiPolicy {
    pub removed: bool,
}

pub async fn delete_ai_policy(
    State(state): State<AppState>,
    Query(selector): Query<AiPolicySelector>,
) -> ApiResult<Json<DeletedAiPolicy>> {
    let subject_id = resolve_subject(selector.scope, selector.subject_id.as_deref())?;
    let removed = state
        .db
        .ai_policies()
        .delete(project_id(&state).await?, selector.scope, &subject_id)
        .await?;
    Ok(Json(DeletedAiPolicy { removed }))
}

async fn project_id(state: &AppState) -> Result<Uuid, ApiError> {
    state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or_else(|| ApiError::from(ConductorError::SetupRequired))
}

fn resolve_subject(scope: AiPolicyScope, subject_id: Option<&str>) -> Result<String, ApiError> {
    let subject = subject_id.filter(|value| !value.is_empty());
    match (scope, subject) {
        (AiPolicyScope::Project, None) => Ok(String::new()),
        (AiPolicyScope::Project, Some(_)) => Err(ConductorError::msg(
            "a project policy covers everyone and takes no subject_id",
        )
        .into()),
        (AiPolicyScope::Role, None) => {
            Err(ConductorError::msg("subject_id is required for a role policy").into())
        }
        (AiPolicyScope::Role, Some(value)) => PrimaryRole::parse(value)
            .map(|role| role.as_str().to_string())
            .ok_or_else(|| {
                ApiError::from(ConductorError::msg(format!(
                    "subject_id must be one of {}",
                    PrimaryRole::ALL
                        .iter()
                        .map(|role| role.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )))
            }),
    }
}

fn to_view(policy: AiPolicy) -> AiPolicyView {
    AiPolicyView {
        scope: policy.scope,
        subject_id: policy.subject_id,
        default_provider: policy.default_provider,
        default_model: policy.default_model,
        allowed_providers: policy.allowed_providers,
        allowed_tools: policy.allowed_tools,
    }
}
