use std::collections::HashSet;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{Duration, Utc};
use conductor_domain::{
    ConductorError, CreateResourceRequest, CreateResourceVersionRequest, ManagedResource,
    PrimaryRole, ResourceAccessPolicy, ResourceFeedback, ResourceMonitoring, ResourceStatus,
    ResourceUsageBatchRequest, ResourceUsageBatchResponse, ResourceUsageRejection, ResourceVersion,
    ResourceVisibility, SecretScope, UpdateResourceRequest, UpsertResourceFeedbackRequest,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::error::{ApiError, ApiResult};
use crate::core::state::AppState;
use crate::http::extractors::{authenticate_connection_secret, AuthUser};
use crate::http::realtime::{RealtimeAudience, RealtimeSignal};

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Vec<ManagedResource>>> {
    Ok(Json(
        state
            .db
            .resources()
            .list_for_actor(user.id, user.primary_role)
            .await?,
    ))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Json(mut request): Json<CreateResourceRequest>,
) -> ApiResult<Json<ManagedResource>> {
    require_catalog_manager(actor.primary_role)?;
    normalize_create_request(&mut request);
    validate_resource_request(
        &request.slug,
        &request.name,
        request.description.as_deref(),
        &request.version,
        &request.payload,
        request.changelog.as_deref(),
    )?;

    match state.db.resources().create(&request, actor.id).await {
        Ok(resource) => Ok(Json(resource)),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(ConductorError::Conflict("kind and slug already exist".into()).into())
        }
        Err(error) => Err(ApiError::from(error)),
    }
}

pub async fn update(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Json(mut request): Json<UpdateResourceRequest>,
) -> ApiResult<Json<ManagedResource>> {
    let existing = managed_resource(&state, &actor, resource_id).await?;
    if let Some(name) = request.name.as_mut() {
        *name = name.trim().to_string();
        if name.is_empty() || name.len() > 120 {
            return Err(ConductorError::msg("name must be 1–120 characters").into());
        }
    }
    if request
        .description
        .as_ref()
        .is_some_and(|description| description.len() > 1_000)
    {
        return Err(ConductorError::msg("description must be at most 1000 characters").into());
    }

    let resource = state
        .db
        .resources()
        .update(resource_id, &request)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
    if existing.status == ResourceStatus::Published {
        publish_catalog_state(&state, &resource).await?;
    }
    Ok(Json(resource))
}

pub async fn archive(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
) -> ApiResult<Json<ManagedResource>> {
    let _ = managed_resource(&state, &actor, resource_id).await?;
    if !state.db.resources().archive(resource_id).await? {
        return Err(ConductorError::Conflict("resource is already archived".into()).into());
    }
    state.realtime.publish(RealtimeSignal::ResourceDelete {
        audience: RealtimeAudience::All,
        resource_id,
    });
    Ok(Json(
        state
            .db
            .resources()
            .find_by_id(resource_id)
            .await?
            .ok_or_else(|| ConductorError::NotFound("resource".into()))?,
    ))
}

pub async fn versions(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ResourceVersion>>> {
    let _ = managed_resource(&state, &actor, resource_id).await?;
    Ok(Json(state.db.resources().versions(resource_id).await?))
}

pub async fn create_version(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Json(mut request): Json<CreateResourceVersionRequest>,
) -> ApiResult<Json<ResourceVersion>> {
    let _ = managed_resource(&state, &actor, resource_id).await?;
    request.version = request.version.trim().to_string();
    request.changelog = clean_optional(request.changelog);
    validate_version(&request.version)?;
    validate_payload(&request.payload)?;
    if request
        .changelog
        .as_ref()
        .is_some_and(|changelog| changelog.len() > 2_000)
    {
        return Err(ConductorError::msg("changelog must be at most 2000 characters").into());
    }

    match state
        .db
        .resources()
        .create_version(resource_id, &request, actor.id)
        .await
    {
        Ok(version) => Ok(Json(version)),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(ConductorError::Conflict("version already exists".into()).into())
        }
        Err(error) => Err(ApiError::from(error)),
    }
}

pub async fn publish_version(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path((resource_id, version_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ManagedResource>> {
    let _ = managed_resource(&state, &actor, resource_id).await?;
    let resource = state
        .db
        .resources()
        .publish_version(resource_id, version_id)
        .await?
        .ok_or_else(|| ConductorError::Conflict("only a draft version can be published".into()))?;
    publish_catalog_state(&state, &resource).await?;
    Ok(Json(resource))
}

pub async fn get_access(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
) -> ApiResult<Json<ResourceAccessPolicy>> {
    let _ = managed_resource(&state, &actor, resource_id).await?;
    Ok(Json(state.db.resources().access_policy(resource_id).await?))
}

pub async fn set_access(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Json(mut policy): Json<ResourceAccessPolicy>,
) -> ApiResult<Json<ResourceAccessPolicy>> {
    let resource = managed_resource(&state, &actor, resource_id).await?;
    normalize_policy(&mut policy);
    validate_policy(&state, &policy).await?;
    state
        .db
        .resources()
        .set_access_policy(resource_id, &policy)
        .await?;
    if resource.status == ResourceStatus::Published {
        publish_catalog_state(&state, &resource).await?;
    }
    Ok(Json(policy))
}

#[derive(Debug, Deserialize)]
pub struct MonitoringQuery {
    pub days: Option<u32>,
}

pub async fn monitoring(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Query(query): Query<MonitoringQuery>,
) -> ApiResult<Json<ResourceMonitoring>> {
    let _ = managed_resource(&state, &actor, resource_id).await?;
    let days = query.days.unwrap_or(30).clamp(7, 90);
    Ok(Json(
        state.db.resources().monitoring(resource_id, days).await?,
    ))
}

pub async fn feedback(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ResourceFeedback>>> {
    let _ = managed_resource(&state, &actor, resource_id).await?;
    Ok(Json(state.db.resources().feedback(resource_id).await?))
}

pub async fn upsert_feedback(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(resource_id): Path<Uuid>,
    Json(mut request): Json<UpsertResourceFeedbackRequest>,
) -> ApiResult<Json<ResourceFeedback>> {
    if !(1..=5).contains(&request.rating) {
        return Err(ConductorError::msg("rating must be between 1 and 5").into());
    }
    request.comment = clean_optional(request.comment);
    if request
        .comment
        .as_ref()
        .is_some_and(|comment| comment.len() > 1_000)
    {
        return Err(ConductorError::msg("comment must be at most 1000 characters").into());
    }
    let visible_ids = state.db.resources().visible_resource_ids(actor.id).await?;
    if !visible_ids.contains(&resource_id) {
        return Err(ConductorError::NotFound("resource".into()).into());
    }
    let resource = state
        .db
        .resources()
        .find_by_id(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
    Ok(Json(
        state
            .db
            .resources()
            .upsert_feedback(&resource, actor.id, &request)
            .await?,
    ))
}

/// EvoFlux resource snapshot fallback — `Authorization: Bearer evc_...`.
pub async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ManagedResource>>> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::SubscribeResources).await?;
    Ok(Json(
        state
            .db
            .resources()
            .list_visible_to(principal.secret.owner_user_id)
            .await?,
    ))
}

/// Idempotent EvoFlux usage batch. Member identity always comes from the secret owner.
pub async fn ingest_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ResourceUsageBatchRequest>,
) -> ApiResult<Json<ResourceUsageBatchResponse>> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::ReportTelemetry).await?;
    if request.events.is_empty() || request.events.len() > 100 {
        return Err(ConductorError::msg("events batch must contain 1–100 items").into());
    }

    let visible_ids = state
        .db
        .resources()
        .visible_resource_ids(principal.user.id)
        .await?;
    let requested_versions: HashSet<_> = request
        .events
        .iter()
        .map(|event| (event.resource_id, event.resource_version.trim().to_string()))
        .collect();
    let existing_versions = state
        .db
        .resources()
        .existing_versions(&requested_versions)
        .await?;
    let mut response = ResourceUsageBatchResponse {
        accepted: 0,
        duplicates: 0,
        rejected: 0,
        rejections: vec![],
    };
    let now = Utc::now();
    let oldest = now - Duration::days(90);
    let newest = now + Duration::minutes(5);

    for event in request.events {
        let version_key = (event.resource_id, event.resource_version.trim().to_string());
        let rejection = if !visible_ids.contains(&event.resource_id) {
            Some("resource_not_accessible")
        } else if validate_version(&event.resource_version).is_err()
            || !existing_versions.contains(&version_key)
        {
            Some("unknown_resource_version")
        } else if event.occurred_at < oldest || event.occurred_at > newest {
            Some("timestamp_out_of_range")
        } else if event.duration_ms > 86_400_000 {
            Some("duration_out_of_range")
        } else if event.tokens_in > 100_000_000 || event.tokens_out > 100_000_000 {
            Some("token_count_out_of_range")
        } else if event
            .session_id
            .as_ref()
            .is_some_and(|session_id| session_id.len() > 120)
        {
            Some("session_id_too_long")
        } else {
            None
        };
        if let Some(reason) = rejection {
            response.rejected += 1;
            response.rejections.push(ResourceUsageRejection {
                event_id: event.event_id,
                reason: reason.to_string(),
            });
            continue;
        }
        if state
            .db
            .resources()
            .insert_usage_event(principal.user.id, &event)
            .await?
        {
            response.accepted += 1;
        } else {
            response.duplicates += 1;
        }
    }
    Ok(Json(response))
}

async fn managed_resource(
    state: &AppState,
    actor: &conductor_domain::User,
    resource_id: Uuid,
) -> ApiResult<ManagedResource> {
    require_catalog_manager(actor.primary_role)?;
    let resource = state
        .db
        .resources()
        .find_by_id(resource_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
    if actor.primary_role != PrimaryRole::Admin && resource.owner_user_id != Some(actor.id) {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(resource)
}

fn require_catalog_manager(role: PrimaryRole) -> ApiResult<()> {
    if !role.can_manage_resources() {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(())
}

async fn publish_catalog_state(state: &AppState, resource: &ManagedResource) -> ApiResult<()> {
    state.realtime.publish(RealtimeSignal::ResourceDelete {
        audience: RealtimeAudience::All,
        resource_id: resource.id,
    });
    if resource.status != ResourceStatus::Published {
        return Ok(());
    }

    let policy = state.db.resources().access_policy(resource.id).await?;
    let no_explicit_rules = !policy.all_members
        && policy.primary_roles.is_empty()
        && policy.sub_role_ids.is_empty()
        && policy.tag_ids.is_empty()
        && policy.member_ids.is_empty();
    let audience = if resource.visibility == ResourceVisibility::Shared && no_explicit_rules
        || policy.all_members
    {
        RealtimeAudience::All
    } else if no_explicit_rules {
        RealtimeAudience::Owner(resource.owner_user_id.unwrap_or_else(Uuid::nil))
    } else {
        RealtimeAudience::Policy {
            owner_user_id: resource.owner_user_id.unwrap_or_else(Uuid::nil),
            policy,
        }
    };
    state.realtime.publish(RealtimeSignal::ResourceUpsert {
        audience,
        resource: resource.clone(),
    });
    Ok(())
}

async fn validate_policy(state: &AppState, policy: &ResourceAccessPolicy) -> ApiResult<()> {
    if policy.primary_roles.len() > 3
        || policy.sub_role_ids.len() > 100
        || policy.tag_ids.len() > 100
        || policy.member_ids.len() > 100
    {
        return Err(ConductorError::msg("access policy has too many subjects").into());
    }
    if policy
        .primary_roles
        .iter()
        .any(|role| PrimaryRole::parse(role).is_none())
        || !state
            .db
            .roles()
            .all_sub_roles_exist(&policy.sub_role_ids)
            .await?
        || !state.db.roles().all_tags_exist(&policy.tag_ids).await?
        || !state.db.users().all_users_exist(&policy.member_ids).await?
    {
        return Err(ConductorError::msg("access policy contains unknown subjects").into());
    }
    Ok(())
}

fn normalize_policy(policy: &mut ResourceAccessPolicy) {
    policy.primary_roles.sort();
    policy.primary_roles.dedup();
    policy.sub_role_ids.sort();
    policy.sub_role_ids.dedup();
    policy.tag_ids.sort();
    policy.tag_ids.dedup();
    policy.member_ids.sort();
    policy.member_ids.dedup();
}

fn normalize_create_request(request: &mut CreateResourceRequest) {
    request.slug = request.slug.trim().to_lowercase();
    request.name = request.name.trim().to_string();
    request.version = request.version.trim().to_string();
    request.description = clean_optional(request.description.take());
    request.changelog = clean_optional(request.changelog.take());
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_resource_request(
    slug: &str,
    name: &str,
    description: Option<&str>,
    version: &str,
    payload: &serde_json::Value,
    changelog: Option<&str>,
) -> ApiResult<()> {
    if slug.is_empty()
        || slug.len() > 80
        || !slug.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        return Err(ConductorError::msg(
            "slug must use 1–80 lowercase letters, numbers, or hyphens",
        )
        .into());
    }
    if name.is_empty() || name.len() > 120 {
        return Err(ConductorError::msg("name must be 1–120 characters").into());
    }
    if description.is_some_and(|description| description.len() > 1_000) {
        return Err(ConductorError::msg("description must be at most 1000 characters").into());
    }
    if changelog.is_some_and(|changelog| changelog.len() > 2_000) {
        return Err(ConductorError::msg("changelog must be at most 2000 characters").into());
    }
    validate_version(version)?;
    validate_payload(payload)
}

fn validate_version(version: &str) -> ApiResult<()> {
    if version.is_empty() || version.len() > 64 {
        return Err(ConductorError::msg("version must be valid semantic version text").into());
    }
    let core = version.split_once('-').map_or(version, |(core, _)| core);
    let parts: Vec<_> = core.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty() || !part.chars().all(|character| character.is_ascii_digit())
        })
    {
        return Err(ConductorError::msg("version must follow major.minor.patch").into());
    }
    Ok(())
}

fn validate_payload(payload: &serde_json::Value) -> ApiResult<()> {
    let size = serde_json::to_vec(payload)
        .map_err(|_| ConductorError::msg("invalid payload"))?
        .len();
    if size > 256 * 1024 {
        return Err(ConductorError::msg("payload must be at most 256 KiB").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_semantic_versions() {
        assert!(validate_version("1.2.3").is_ok());
        assert!(validate_version("1.2.3-beta.1").is_ok());
        assert!(validate_version("1.2").is_err());
        assert!(validate_version("latest").is_err());
    }

    #[test]
    fn normalizes_duplicate_policy_subjects() {
        let mut policy = ResourceAccessPolicy {
            member_ids: vec![Uuid::nil(), Uuid::nil()],
            primary_roles: vec!["user".into(), "user".into()],
            ..ResourceAccessPolicy::default()
        };
        normalize_policy(&mut policy);
        assert_eq!(policy.member_ids.len(), 1);
        assert_eq!(policy.primary_roles.len(), 1);
    }
}
