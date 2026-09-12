use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use chrono::{DateTime, Duration, Utc};
use conductor_domain::{
    AuthorizationTarget, ConductorError, MemberActivityResponse, MemberCostReport,
    MemberRequestDetail, MemberUsageSummary, ModelCostReport, PrimaryRole, ResourceKind,
    ResourceUsageAnalytics, ResourceUsageScope, ResponseProjection, TargetType,
    TelemetryBatchRequest, TelemetryBatchResponse, TelemetryEventRequest, TelemetryEventStatus,
    TelemetryEventType, TelemetryResourceRelation, TokenUsage, RETIRED_TELEMETRY_EVENT_FIELDS,
};
use conductor_storage::repos::{CostReportFilters, PricedTelemetryEvent, ResourceUsageQuery};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::constants::telemetry::{
    DEFAULT_ACTIVITY_LIMIT, DEFAULT_RANGE_DAYS, MAX_ACTIVITY_LIMIT, MAX_BATCH_SIZE,
    MAX_DURATION_MS_PER_EVENT, MAX_FUTURE_CLOCK_SKEW_MINUTES, MAX_LABEL_LENGTH,
    MAX_RESOURCE_ATTRIBUTIONS_PER_EVENT, MAX_TOKENS_PER_EVENT, MIN_ACTIVITY_LIMIT,
    MIN_LABEL_LENGTH,
};
use crate::core::error::ApiResult;
use crate::core::model_pricing;
use crate::core::state::AppState;
use crate::http::authorization::{
    authorize_current_browser_target, authorize_current_browser_target_with_aggregate_fact,
    authorize_current_connection_target, RouteAuthorization,
};
use crate::http::extractors::{AuthUser, ConnectionPrincipal};

#[derive(Debug, Deserialize)]
pub struct RangeQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

/// Shared by the model and member cost reports: a window plus the optional
/// narrowing "monitoring" filters -- role, tag, provider or model.
#[derive(Debug, Deserialize)]
pub struct CostReportQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub primary_role: Option<PrimaryRole>,
    pub tag_id: Option<Uuid>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

impl CostReportQuery {
    fn into_filters(self) -> CostReportFilters {
        CostReportFilters {
            primary_role: self.primary_role,
            tag_id: self.tag_id,
            provider: self.provider,
            model: self.model,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ResourceAnalyticsQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub member_id: Option<Uuid>,
    pub primary_role: Option<PrimaryRole>,
    pub resource_kind: Option<ResourceKind>,
    pub resource_id: Option<Uuid>,
    pub version_id: Option<Uuid>,
    pub status: Option<TelemetryEventStatus>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub installation_id: Option<Uuid>,
    pub relation: Option<TelemetryResourceRelation>,
    pub scope: Option<ResourceUsageScope>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn ingest(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    Extension(principal): Extension<ConnectionPrincipal>,
    Json(request): Json<TelemetryBatchRequest>,
) -> ApiResult<Json<TelemetryBatchResponse>> {
    if request.events.len() > MAX_BATCH_SIZE {
        return Err(ConductorError::msg(format!(
            "events must contain at most {MAX_BATCH_SIZE} items"
        ))
        .into());
    }
    let installation = state
        .db
        .client_installations()
        .find_by_id(request.installation_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("client installation".into()))?;
    let instance = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?;
    if let Err(error) = authorize_current_connection_target(
        &state,
        &route,
        &principal,
        AuthorizationTarget {
            project_id: Some(installation.instance_id),
            target_type: TargetType::ClientInstallation,
            target_id: Some(installation.id),
            owner_id: Some(installation.user_id),
            resource_kind: None,
            lifecycle: None,
            effective_audience: None,
        },
    )
    .await
    {
        if installation.user_id != principal.user.id || installation.instance_id != instance.id {
            return Err(ConductorError::NotFound("client installation".into()).into());
        }
        return Err(error);
    }
    if installation.instance_id != instance.id {
        return Err(ConductorError::NotFound("client installation".into()).into());
    }
    if !conductor_domain::CollectionLevel::parse(&state.db.instance().collection_level().await?)
        .telemetry_enabled()
    {
        return Err(ConductorError::Forbidden.into());
    }
    if request.events.is_empty() {
        let to = Utc::now();
        let window_days = DEFAULT_RANGE_DAYS as u16;
        let summary = state
            .db
            .telemetry()
            .delivery_summary(
                instance.id,
                principal.user.id,
                request.installation_id,
                to - Duration::days(DEFAULT_RANGE_DAYS),
                to,
                window_days,
            )
            .await?;
        return Ok(Json(TelemetryBatchResponse {
            accepted: 0,
            duplicates: 0,
            summary: Some(summary),
        }));
    }
    for event in &request.events {
        validate_event(event)?;
    }
    let visible_resources = state
        .db
        .resources()
        .visible_resource_ids(principal.user.id)
        .await?;
    for event in &request.events {
        for reference in &event.resources {
            if !visible_resources.contains(&reference.resource_id) {
                return Err(ConductorError::Forbidden.into());
            }
            let resource = state
                .db
                .resources()
                .find_by_id(reference.resource_id)
                .await?
                .filter(|resource| resource.project_id == instance.id)
                .ok_or_else(|| ConductorError::NotFound("resource".into()))?;
            if !state
                .db
                .resources()
                .version_belongs_to(reference.resource_id, reference.version_id)
                .await?
            {
                return Err(ConductorError::NotFound("resource version".into()).into());
            }
            let relation_matches_kind = match reference.relation {
                // The Agent that ran is attributed to the governed Team it
                // belongs to, which is the unit Conductor publishes.
                TelemetryResourceRelation::ExecutingAgent => {
                    resource.kind.as_str() == "agent_team"
                }
                TelemetryResourceRelation::ActivatedSkill => resource.kind.as_str() == "skill",
                TelemetryResourceRelation::PluginContributedSkill
                | TelemetryResourceRelation::PluginContributedTool => {
                    resource.kind.as_str() == "plugin"
                }
            };
            if !relation_matches_kind {
                return Err(
                    ConductorError::msg("resource relation does not match resource kind").into(),
                );
            }
            if matches!(
                reference.relation,
                TelemetryResourceRelation::PluginContributedSkill
                    | TelemetryResourceRelation::PluginContributedTool
            ) {
                let plugin_installation_id =
                    reference.plugin_installation_id.as_deref().ok_or_else(|| {
                        ConductorError::msg("plugin attribution requires plugin_installation_id")
                    })?;
                if !state
                    .db
                    .resources()
                    .inventory_plugin_matches(
                        instance.id,
                        request.installation_id,
                        reference.resource_id,
                        reference.version_id,
                        plugin_installation_id,
                    )
                    .await?
                {
                    return Err(ConductorError::Forbidden.into());
                }
            }
        }
    }

    // Conductor prices every call from its own rate table. Clients report
    // usage and never cost, so there is one costing method behind every
    // figure; a call Conductor cannot price stays unpriced rather than
    // falling back to a number computed somewhere else.
    let mut priced = Vec::with_capacity(request.events.len());
    for event in &request.events {
        let (cost, catalog_version) = if event.event_type == TelemetryEventType::ModelCall {
            model_pricing::price_event(
                &state.db,
                &state.model_rates,
                event.provider.as_deref(),
                event.model.as_deref(),
                TokenUsage {
                    tokens_in: to_signed(event.tokens_in),
                    tokens_out: to_signed(event.tokens_out),
                    cache_read: to_signed(event.cache_read_tokens),
                    cache_write: to_signed(event.cache_write_tokens),
                    reasoning: to_signed(event.reasoning_tokens),
                },
                event.service_tier.as_deref(),
                event.reported_at,
            )
            .await?
        } else {
            // Only model calls consume tokens; anything else has no price.
            (
                conductor_domain::PricedCost::Unpriced {
                    reason: conductor_domain::UnpricedReason::NoMatchingComponent,
                },
                None,
            )
        };
        // Ingest always prices from the rate in force; the estimate basis
        // exists only for the reprice pass over pre-catalog history.
        let basis = cost
            .cost_micros()
            .map(|_| conductor_domain::PricingBasis::InForce);
        priced.push(PricedTelemetryEvent {
            event,
            cost,
            catalog_version,
            basis,
        });
    }

    Ok(Json(
        state
            .db
            .telemetry()
            .ingest(
                instance.id,
                &principal.user,
                request.installation_id,
                &priced,
            )
            .await?,
    ))
}

/// Token counters are validated against `MAX_TOKENS_PER_EVENT` before this,
/// so the cast cannot lose a meaningful value.
fn to_signed(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

pub async fn usage_summary(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(user_id): Path<Uuid>,
    Query(query): Query<RangeQuery>,
) -> ApiResult<Json<MemberUsageSummary>> {
    ensure_member_access(&state, &route, &actor, user_id).await?;
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
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ActivityQuery>,
) -> ApiResult<Json<MemberActivityResponse>> {
    ensure_member_access(&state, &route, &actor, user_id).await?;
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
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path((user_id, request_id)): Path<(Uuid, String)>,
) -> ApiResult<Json<MemberRequestDetail>> {
    ensure_member_access(&state, &route, &actor, user_id).await?;
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

pub async fn resource_usage(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Query(query): Query<ResourceAnalyticsQuery>,
) -> ApiResult<Json<ResourceUsageAnalytics>> {
    let scope = query.scope.unwrap_or_default();
    if scope == ResourceUsageScope::All
        && (query.resource_kind.is_some()
            || query.resource_id.is_some()
            || query.version_id.is_some()
            || query.relation.is_some())
    {
        return Err(ConductorError::msg(
            "resource filters require scope=governed because ordinary activity has no resource attribution",
        )
        .into());
    }
    let instance = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?;
    let identifying_filter = query.member_id.is_some() || query.installation_id.is_some();
    let decision = authorize_current_browser_target_with_aggregate_fact(
        &state,
        &route,
        &actor,
        AuthorizationTarget {
            project_id: Some(instance.id),
            target_type: TargetType::Project,
            target_id: query.member_id.or(query.installation_id),
            owner_id: None,
            resource_kind: query.resource_kind,
            lifecycle: None,
            effective_audience: None,
        },
        !identifying_filter,
    )
    .await?;
    for (name, value) in [
        ("provider", query.provider.as_deref()),
        ("model", query.model.as_deref()),
    ] {
        if value.is_some_and(|value| value.is_empty() || value.len() > MAX_LABEL_LENGTH) {
            return Err(ConductorError::msg(format!(
                "{name} must be {MIN_LABEL_LENGTH}–{MAX_LABEL_LENGTH} characters when provided"
            ))
            .into());
        }
    }
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    let mut analytics = state
        .db
        .resource_usage()
        .analytics(&ResourceUsageQuery {
            project_id: instance.id,
            from,
            to,
            user_id: query.member_id,
            primary_role: query.primary_role,
            resource_kind: query.resource_kind,
            resource_id: query.resource_id,
            version_id: query.version_id,
            status: query.status,
            provider: query.provider,
            model: query.model,
            installation_id: query.installation_id,
            relation: query.relation,
            scope,
            limit: query
                .limit
                .unwrap_or(DEFAULT_ACTIVITY_LIMIT)
                .clamp(MIN_ACTIVITY_LIMIT, MAX_ACTIVITY_LIMIT),
            offset: query.offset.unwrap_or(0),
        })
        .await?;
    if decision.response_projection == Some(ResponseProjection::AggregateOnly) {
        // These are the only panels in this DTO containing member or request
        // identity. Contributors receive the remaining project aggregates.
        analytics.members.clear();
        analytics.activity.clear();
        analytics.activity_total = 0;
    }
    Ok(Json(analytics))
}

async fn ensure_member_access(
    state: &AppState,
    route: &RouteAuthorization,
    actor: &conductor_domain::User,
    user_id: Uuid,
) -> ApiResult<()> {
    let member = state
        .db
        .users()
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
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
            target_id: Some(member.id),
            owner_id: Some(member.id),
            resource_kind: None,
            lifecycle: None,
            effective_audience: None,
        },
    )
    .await?;
    Ok(())
}

/// Project-wide spend by model, split into the token components a
/// models.dev rate prices separately. See `core::model_cost_report`.
pub async fn model_cost_report(
    State(state): State<AppState>,
    Query(query): Query<CostReportQuery>,
) -> ApiResult<Json<ModelCostReport>> {
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    let filters = CostReportQuery {
        from: None,
        to: None,
        ..query
    }
    .into_filters();
    let project_id = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?
        .id;
    Ok(Json(
        crate::core::model_cost_report::build(
            &state.db,
            &state.model_rates,
            project_id,
            from,
            to,
            &filters,
        )
        .await?,
    ))
}

/// Project-wide spend by member. See `core::member_cost_report`.
pub async fn member_cost_report(
    State(state): State<AppState>,
    Query(query): Query<CostReportQuery>,
) -> ApiResult<Json<MemberCostReport>> {
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    let filters = CostReportQuery {
        from: None,
        to: None,
        ..query
    }
    .into_filters();
    let project_id = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?
        .id;
    Ok(Json(
        crate::core::member_cost_report::build(&state.db, project_id, from, to, &filters).await?,
    ))
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
    // Retired fields are tolerated so older installations keep reporting.
    // Everything else unknown is refused: the schema's strictness is what
    // stops a client sending conversation content under an invented key, and
    // that guarantee is not the one being relaxed.
    if let Some(name) = event
        .unknown
        .keys()
        .find(|key| !RETIRED_TELEMETRY_EVENT_FIELDS.contains(&key.as_str()))
    {
        return Err(ConductorError::msg(format!("unknown telemetry field: {name}")).into());
    }
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
        ("response_model", event.response_model.as_deref()),
        ("error_category", event.error_category.as_deref()),
        ("service_tier", event.service_tier.as_deref()),
    ] {
        if value.is_some_and(|value| value.is_empty() || value.len() > MAX_LABEL_LENGTH) {
            return Err(ConductorError::msg(format!(
                "{name} must be {MIN_LABEL_LENGTH}–{MAX_LABEL_LENGTH} characters when provided"
            ))
            .into());
        }
    }
    if event.resources.len() > MAX_RESOURCE_ATTRIBUTIONS_PER_EVENT {
        return Err(ConductorError::msg(format!(
            "resources cannot contain more than {MAX_RESOURCE_ATTRIBUTIONS_PER_EVENT} items"
        ))
        .into());
    }
    let mut resource_refs = std::collections::HashSet::new();
    if event.resources.iter().any(|item| {
        !resource_refs.insert((item.resource_id, item.version_id, item.relation.as_str()))
    }) {
        return Err(ConductorError::msg("resources contains duplicate attributions").into());
    }
    for (name, value) in [
        ("tokens_in", event.tokens_in),
        ("tokens_out", event.tokens_out),
        ("cache_read_tokens", event.cache_read_tokens),
        ("cache_write_tokens", event.cache_write_tokens),
        ("reasoning_tokens", event.reasoning_tokens),
        ("tool_use_tokens", event.tool_use_tokens),
    ] {
        if value > MAX_TOKENS_PER_EVENT {
            return Err(ConductorError::msg(format!(
                "{name} cannot exceed {MAX_TOKENS_PER_EVENT} per event"
            ))
            .into());
        }
    }
    if event.duration_ms > MAX_DURATION_MS_PER_EVENT {
        return Err(ConductorError::msg(format!(
            "duration_ms cannot exceed {MAX_DURATION_MS_PER_EVENT} per event"
        ))
        .into());
    }
    if event.reported_at > Utc::now() + Duration::minutes(MAX_FUTURE_CLOCK_SKEW_MINUTES) {
        return Err(ConductorError::msg("reported_at is too far in the future").into());
    }
    Ok(())
}
