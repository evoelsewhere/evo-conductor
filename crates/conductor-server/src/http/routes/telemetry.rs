use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use conductor_domain::{
    ConductorError, MemberActivityResponse, MemberRequestDetail, MemberToolsSummary,
    MemberUsageSummary, SecretScope, TelemetryBatchRequest, TelemetryBatchResponse,
    TelemetryEventRequest, TelemetryEventType,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::constants::telemetry::{
    DEFAULT_ACTIVITY_LIMIT, DEFAULT_RANGE_DAYS, MAX_ACTIVITY_LIMIT, MAX_BATCH_SIZE,
    MAX_FUTURE_CLOCK_SKEW_MINUTES, MAX_LABEL_LENGTH, MIN_ACTIVITY_LIMIT, MIN_BATCH_SIZE,
    MIN_LABEL_LENGTH,
};
use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::{authenticate_connection_secret, AuthUser};

#[derive(Debug, Deserialize)]
pub struct RangeQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TelemetryBatchRequest>,
) -> ApiResult<Json<TelemetryBatchResponse>> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::ReportTelemetry).await?;
    if request.events.len() < MIN_BATCH_SIZE || request.events.len() > MAX_BATCH_SIZE {
        return Err(ConductorError::msg(format!(
            "events must contain {MIN_BATCH_SIZE}–{MAX_BATCH_SIZE} items"
        ))
        .into());
    }
    let installation = state
        .db
        .client_installations()
        .find_by_id(request.installation_id)
        .await?
        .filter(|item| item.user_id == principal.user.id)
        .ok_or_else(|| ConductorError::NotFound("client installation".into()))?;
    let instance = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?;
    if installation.instance_id != instance.id {
        return Err(ConductorError::NotFound("client installation".into()).into());
    }
    if !conductor_domain::CollectionLevel::parse(&state.db.instance().collection_level().await?)
        .telemetry_enabled()
    {
        return Err(ConductorError::Forbidden.into());
    }
    for event in &request.events {
        validate_event(event)?;
    }

    Ok(Json(
        state
            .db
            .telemetry()
            .ingest(principal.user.id, request.installation_id, &request.events)
            .await?,
    ))
}

pub async fn usage_summary(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(user_id): Path<Uuid>,
    Query(query): Query<RangeQuery>,
) -> ApiResult<Json<MemberUsageSummary>> {
    ensure_member_access(&state, &actor, user_id).await?;
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    Ok(Json(
        state
            .db
            .telemetry()
            .usage_summary(user_id, from, to)
            .await?,
    ))
}

pub async fn activity(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ActivityQuery>,
) -> ApiResult<Json<MemberActivityResponse>> {
    ensure_member_access(&state, &actor, user_id).await?;
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    Ok(Json(
        state
            .db
            .telemetry()
            .activity(
                user_id,
                from,
                to,
                query
                    .limit
                    .unwrap_or(DEFAULT_ACTIVITY_LIMIT)
                    .clamp(MIN_ACTIVITY_LIMIT, MAX_ACTIVITY_LIMIT),
                query.offset.unwrap_or(0),
            )
            .await?,
    ))
}

pub async fn request_detail(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path((user_id, request_id)): Path<(Uuid, String)>,
) -> ApiResult<Json<MemberRequestDetail>> {
    ensure_member_access(&state, &actor, user_id).await?;
    if request_id.is_empty() || request_id.len() > MAX_LABEL_LENGTH {
        return Err(ConductorError::NotFound("request".into()).into());
    }
    Ok(Json(
        state
            .db
            .telemetry()
            .request_detail(user_id, &request_id)
            .await?
            .ok_or_else(|| ConductorError::NotFound("request".into()))?,
    ))
}

pub async fn tools_summary(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(user_id): Path<Uuid>,
    Query(query): Query<RangeQuery>,
) -> ApiResult<Json<MemberToolsSummary>> {
    ensure_member_access(&state, &actor, user_id).await?;
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    Ok(Json(
        state
            .db
            .telemetry()
            .tools_summary(user_id, from, to)
            .await?,
    ))
}

async fn ensure_member_access(
    state: &AppState,
    actor: &conductor_domain::User,
    user_id: Uuid,
) -> ApiResult<()> {
    if actor.id != user_id && !actor.primary_role.can_view_telemetry() {
        return Err(ConductorError::Forbidden.into());
    }
    state
        .db
        .users()
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    Ok(())
}

fn resolve_range(
    from: Option<&str>,
    to: Option<&str>,
) -> ApiResult<(DateTime<Utc>, DateTime<Utc>)> {
    let to = match to {
        Some(value) => parse_timestamp(value, "to")?,
        None => Utc::now(),
    };
    let from = match from {
        Some(value) => parse_timestamp(value, "from")?,
        None => to - Duration::days(DEFAULT_RANGE_DAYS),
    };
    if from > to {
        return Err(ConductorError::msg("from must be before to").into());
    }
    Ok((from, to))
}

fn parse_timestamp(value: &str, field: &str) -> ApiResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| ConductorError::msg(format!("{field} must be an RFC 3339 timestamp")).into())
}

fn validate_event(event: &TelemetryEventRequest) -> ApiResult<()> {
    if event.request_id.trim().is_empty() || event.request_id.len() > MAX_LABEL_LENGTH {
        return Err(ConductorError::msg(format!(
            "request_id must be {MIN_LABEL_LENGTH}–{MAX_LABEL_LENGTH} characters"
        ))
        .into());
    }
    for (name, value) in [
        ("session_id", event.session_id.as_deref()),
        ("agent_name", event.agent_name.as_deref()),
        ("provider", event.provider.as_deref()),
        ("model", event.model.as_deref()),
        ("tool_name", event.tool_name.as_deref()),
        ("error_category", event.error_category.as_deref()),
    ] {
        if value.is_some_and(|value| value.is_empty() || value.len() > MAX_LABEL_LENGTH) {
            return Err(ConductorError::msg(format!(
                "{name} must be {MIN_LABEL_LENGTH}–{MAX_LABEL_LENGTH} characters when provided"
            ))
            .into());
        }
    }
    match event.event_type {
        TelemetryEventType::ModelCall if event.tool_name.is_some() => {
            return Err(ConductorError::msg(format!(
                "{} cannot include tool_name",
                TelemetryEventType::ModelCall.as_str()
            ))
            .into());
        }
        TelemetryEventType::ToolCall if event.tool_name.is_none() => {
            return Err(ConductorError::msg(format!(
                "{} requires tool_name",
                TelemetryEventType::ToolCall.as_str()
            ))
            .into());
        }
        _ => {}
    }
    if event.reported_at > Utc::now() + Duration::minutes(MAX_FUTURE_CLOCK_SKEW_MINUTES) {
        return Err(ConductorError::msg("reported_at is too far in the future").into());
    }
    Ok(())
}
