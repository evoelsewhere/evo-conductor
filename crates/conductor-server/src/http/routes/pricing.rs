//! Spend limits and price-catalog operations, for administrators.
//!
//! These are the HTTP face of the `evo-conductor limits` and `reprice`
//! subcommands. The commands stay: an operator locked out of the console still
//! needs them, and repricing a large project is better run from a shell than
//! from a browser tab that may give up first.

use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use conductor_domain::{
    validate_warn_percent, ConductorError, LimitScope, PrimaryRole, SpendLimitListResponse,
    SpendLimitPeriodStatus, SpendLimitSelector, SpendLimitView, UpsertSpendLimitRequest,
};
use conductor_storage::repos::{parse_role_subject, SpendLimit, UpsertSpendLimit};
use serde::Serialize;
use uuid::Uuid;

use crate::core::{model_pricing, reprice, ApiError, ApiResult, AppState};

/// Every configured allowance, with its standing against the current period.
///
/// Disabled limits are included without a status: configured but not
/// evaluated, which is not the same as evaluating to zero.
pub async fn list_spend_limits(
    State(state): State<AppState>,
) -> ApiResult<Json<SpendLimitListResponse>> {
    let project_id = project_id(&state).await?;
    let evaluated_at = Utc::now();
    let limits = state.db.spend_limits();
    let (configured, statuses) = tokio::try_join!(
        limits.list(project_id),
        limits.statuses(project_id, evaluated_at)
    )?;

    let mut views = Vec::with_capacity(configured.len());
    for limit in configured {
        let status = statuses
            .iter()
            .find(|status| {
                status.limit.scope == limit.scope
                    && status.limit.subject_id == limit.subject_id
                    && status.limit.period == limit.period
            })
            .map(|status| SpendLimitPeriodStatus {
                period_start: status.period_start,
                state: status.evaluation.state,
                spent_usd_micros: status.evaluation.spent_usd_micros,
                used_percent: status.evaluation.used_percent,
            });
        views.push(SpendLimitView {
            subject_label: subject_label(&state, &limit).await,
            scope: limit.scope,
            subject_id: limit.subject_id,
            period: limit.period,
            limit_usd_micros: limit.limit_usd_micros,
            warn_percent: limit.warn_percent,
            enabled: limit.enabled,
            status,
        });
    }
    Ok(Json(SpendLimitListResponse {
        limits: views,
        evaluated_at,
    }))
}

pub async fn upsert_spend_limit(
    State(state): State<AppState>,
    Json(request): Json<UpsertSpendLimitRequest>,
) -> ApiResult<Json<SpendLimitView>> {
    validate_warn_percent(request.warn_percent).map_err(ConductorError::msg)?;
    let subject_id = resolve_subject(&state, request.scope, request.subject_id.as_deref()).await?;
    let stored = state
        .db
        .spend_limits()
        .upsert(
            project_id(&state).await?,
            &UpsertSpendLimit {
                scope: request.scope,
                subject_id,
                period: request.period,
                limit_usd_micros: request.limit_usd_micros,
                warn_percent: request.warn_percent,
                enabled: request.enabled,
            },
        )
        .await?;
    Ok(Json(SpendLimitView {
        subject_label: subject_label(&state, &stored).await,
        scope: stored.scope,
        subject_id: stored.subject_id,
        period: stored.period,
        limit_usd_micros: stored.limit_usd_micros,
        warn_percent: stored.warn_percent,
        enabled: stored.enabled,
        status: None,
    }))
}

#[derive(Serialize)]
pub struct DeletedSpendLimit {
    /// False when nothing matched, so a repeated delete is not an error but is
    /// still distinguishable from the first one.
    pub removed: bool,
}

/// The subject is not checked against the member table here: an allowance for
/// someone since removed from the project must still be removable.
pub async fn delete_spend_limit(
    State(state): State<AppState>,
    Query(selector): Query<SpendLimitSelector>,
) -> ApiResult<Json<DeletedSpendLimit>> {
    let subject_id = match (selector.scope, selector.subject_id.as_deref()) {
        (LimitScope::Project, None | Some("")) => String::new(),
        (LimitScope::Project, Some(_)) => {
            return Err(ConductorError::msg(
                "a project limit covers everyone and takes no subject_id",
            )
            .into())
        }
        (_, None | Some("")) => {
            return Err(
                ConductorError::msg("subject_id is required for a member or role limit").into(),
            )
        }
        (_, Some(subject)) => subject.to_string(),
    };
    let removed = state
        .db
        .spend_limits()
        .delete(
            project_id(&state).await?,
            selector.scope,
            &subject_id,
            selector.period,
        )
        .await?;
    Ok(Json(DeletedSpendLimit { removed }))
}

#[derive(Serialize)]
pub struct ModelPricingStatus {
    /// Absent until the first successful sync; every model call ingested
    /// before then carries no Conductor price.
    pub catalog: Option<CatalogView>,
    /// Models the in-force rate table can price right now.
    ///
    /// Cumulative across every sync, not scoped to the latest fetch: a rate
    /// row is kept once inserted so a cost stays explainable by the rate that
    /// was in force when its event was reported (see `apply_catalog`). If
    /// models.dev drops a model between syncs, its last known rate is still
    /// counted here, so this can exceed `catalog.model_count` — the two are
    /// not a part/whole pair and must not be rendered as "X of Y".
    pub priced_models: u64,
    /// Whether syncing is permitted at all for this process.
    pub sync_enabled: bool,
    pub source_url: String,
}

#[derive(Serialize)]
pub struct CatalogView {
    pub version: String,
    pub source: String,
    pub fetched_at: DateTime<Utc>,
    pub model_count: u32,
    pub priced_model_count: u32,
    pub changed_model_count: u32,
}

pub async fn model_pricing_status(
    State(state): State<AppState>,
) -> ApiResult<Json<ModelPricingStatus>> {
    let prices = state.db.model_prices();
    let (catalog, rates) = tokio::try_join!(prices.latest_catalog(), prices.current_rate_table())?;
    Ok(Json(ModelPricingStatus {
        catalog: catalog.map(|snapshot| CatalogView {
            version: snapshot.version,
            source: snapshot.source,
            fetched_at: snapshot.fetched_at,
            model_count: snapshot.model_count,
            priced_model_count: snapshot.priced_model_count,
            changed_model_count: snapshot.changed_model_count,
        }),
        priced_models: rates.len() as u64,
        sync_enabled: state.model_pricing.enabled,
        source_url: state.model_pricing.url.clone(),
    }))
}

/// Fetch the catalog now instead of waiting for the refresh window.
///
/// Reloads the in-memory rate table on success, since ingest prices from that
/// table and would otherwise keep using the rates it started with.
pub async fn sync_model_pricing(State(state): State<AppState>) -> ApiResult<Json<SyncResponse>> {
    if !state.model_pricing.enabled {
        return Err(ConductorError::msg("model pricing sync is disabled for this server").into());
    }
    let outcome = model_pricing::sync_from_models_dev(&state.db, &state.model_pricing.url)
        .await
        .map_err(|error| ConductorError::msg(format!("price catalog sync failed: {error}")))?;
    let reloaded_models = if outcome.unchanged {
        0
    } else {
        state.model_rates.reload(&state.db).await? as u64
    };
    Ok(Json(SyncResponse {
        version: outcome.version,
        model_count: outcome.model_count,
        priced_model_count: outcome.priced_model_count,
        changed_model_count: outcome.changed_model_count,
        unchanged: outcome.unchanged,
        reloaded_models,
    }))
}

#[derive(Serialize)]
pub struct SyncResponse {
    pub version: String,
    pub model_count: u32,
    pub priced_model_count: u32,
    pub changed_model_count: u32,
    /// True when the fetched document matched the last one byte for byte, so
    /// nothing was parsed or written.
    pub unchanged: bool,
    pub reloaded_models: u64,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepriceQuery {
    /// Price events older than the first catalog from the earliest rate on
    /// record. They are marked as estimates, never as prices.
    #[serde(default)]
    pub estimate_pre_catalog: bool,
}

#[derive(Serialize)]
pub struct RepriceResponse {
    pub examined: u64,
    pub priced_in_force: u64,
    pub priced_from_earliest: u64,
    pub left_unpriced: u64,
}

/// Put a cost on model calls Conductor never priced.
///
/// Runs to completion before responding, as the subcommand does. A project
/// with a long unpriced backlog should use `evo-conductor reprice` instead —
/// a browser or proxy may time out first, and repricing is safe to re-run
/// because it only ever fills a cost that is still absent.
pub async fn reprice_model_calls(
    State(state): State<AppState>,
    Query(query): Query<RepriceQuery>,
) -> ApiResult<Json<RepriceResponse>> {
    let basis = if query.estimate_pre_catalog {
        reprice::Basis::EarliestKnownFallback
    } else {
        reprice::Basis::InForceOnly
    };
    let report = reprice::run(&state.db, project_id(&state).await?, basis).await?;
    Ok(Json(RepriceResponse {
        examined: report.examined,
        priced_in_force: report.priced_in_force,
        priced_from_earliest: report.priced_from_earliest,
        left_unpriced: report.left_unpriced,
    }))
}

#[derive(Serialize)]
pub struct ModelCatalogEntry {
    pub provider: String,
    pub model: String,
    pub pricing: conductor_domain::ModelPricing,
}

pub async fn model_pricing_catalog(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<ModelCatalogEntry>>> {
    let rows = state.db.model_prices().current_rate_table().await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| ModelCatalogEntry {
                provider: row.provider,
                model: row.model,
                pricing: row.pricing,
            })
            .collect(),
    ))
}

async fn project_id(state: &AppState) -> Result<Uuid, ApiError> {
    state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or_else(|| ApiError::from(ConductorError::SetupRequired))
}

/// Refuse a subject that names nobody before anything is written.
///
/// A limit on an id no member holds, or a role that is not a role, would
/// report zero spend for ever — indistinguishable from a subject who has not
/// spent.
async fn resolve_subject(
    state: &AppState,
    scope: LimitScope,
    subject_id: Option<&str>,
) -> Result<String, ApiError> {
    let subject = subject_id.filter(|value| !value.is_empty());
    match (scope, subject) {
        (LimitScope::Project, None) => Ok(String::new()),
        (LimitScope::Project, Some(_)) => Err(ConductorError::msg(
            "a project limit covers everyone and takes no subject_id",
        )
        .into()),
        (_, None) => {
            Err(ConductorError::msg("subject_id is required for a member or role limit").into())
        }
        (LimitScope::Role, Some(value)) => parse_role_subject(value).ok_or_else(|| {
            ApiError::from(ConductorError::msg(format!(
                "subject_id must be one of {}",
                PrimaryRole::ALL
                    .iter()
                    .map(|role| role.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }),
        (LimitScope::Member, Some(value)) => {
            let id = Uuid::parse_str(value).map_err(|_| {
                ApiError::from(ConductorError::msg("subject_id must be a member id"))
            })?;
            match state.db.users().find_by_id(id).await? {
                Some(user) => Ok(user.id.to_string()),
                None => Err(ConductorError::NotFound("member".into()).into()),
            }
        }
    }
}

/// Name a member subject so a list does not have to be read as raw ids.
///
/// A member deleted after the limit was created leaves the label absent rather
/// than failing the request; the allowance is still there to be removed.
async fn subject_label(state: &AppState, limit: &SpendLimit) -> Option<String> {
    match limit.scope {
        LimitScope::Project => None,
        LimitScope::Role => Some(limit.subject_id.clone()),
        LimitScope::Member => {
            let id = Uuid::parse_str(&limit.subject_id).ok()?;
            let user = state.db.users().find_by_id(id).await.ok()??;
            Some(user.display_name)
        }
    }
}
