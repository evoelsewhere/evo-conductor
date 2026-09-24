use std::collections::HashMap;

use chrono::{DateTime, Utc};
use conductor_domain::{
    DailyTokenUsage, MemberActivityItem, MemberActivityResponse, MemberRequestDetail,
    MemberUsageSummary, ModelUsageBreakdown, PricedCost, PricingBasis, PrimaryRole, ResourceKind,
    TelemetryBatchResponse, TelemetryDeliverySummary, TelemetryEventDetail, TelemetryEventRequest,
    TelemetryEventStatus, TelemetryEventType, TelemetryResourceAttributionDetail,
    TelemetryResourceRelation, UnpricedReason, User, UNKNOWN_TELEMETRY_LABEL,
};
use sqlx::{Any, Pool, QueryBuilder, Row};
use uuid::Uuid;

use crate::core::mapping::parse_dt;
use crate::DatabaseKind;

/// An event paired with the cost Conductor computed for it.
///
/// One value rather than two parallel slices, so the event and its price
/// cannot silently misalign and attribute one member's cost to another.
#[derive(Debug, Clone)]
pub struct PricedTelemetryEvent<'a> {
    pub event: &'a TelemetryEventRequest,
    pub cost: PricedCost,
    /// The rate snapshot the cost came from; `None` when unpriced.
    pub catalog_version: Option<String>,
    /// Whether the rate actually covered the event, or was an estimate.
    pub basis: Option<PricingBasis>,
}

impl<'a> PricedTelemetryEvent<'a> {
    /// An event Conductor has not priced, for callers that do not price at
    /// all (fixtures, and the legacy paths that only store client figures).
    pub fn unpriced(event: &'a TelemetryEventRequest, reason: UnpricedReason) -> Self {
        Self {
            event,
            cost: PricedCost::Unpriced { reason },
            catalog_version: None,
            basis: None,
        }
    }
}

/// Narrowing shared by the model and member cost reports, so the two always
/// answer for the same population.
#[derive(Debug, Clone, Default)]
pub struct CostReportFilters {
    pub primary_role: Option<PrimaryRole>,
    pub tag_id: Option<Uuid>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

/// Per-model token and cost sums straight from storage, before components
/// are split out against the rate table.
#[derive(Debug, Clone)]
pub struct RawModelCostRow {
    pub provider: String,
    pub model: String,
    pub calls: u64,
    /// Calls Conductor could not price. Their tokens are in the sums above;
    /// their cost is absent rather than zero.
    pub unpriced_calls: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_cost_usd_micros: u64,
}

/// Per-member token and cost sums, with the member's current identity joined
/// in: a cost report names people as they are today, not as they were at call
/// time.
#[derive(Debug, Clone)]
pub struct RawMemberCostRow {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub primary_role: PrimaryRole,
    pub calls: u64,
    pub unpriced_calls: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_cost_usd_micros: u64,
}

/// One raw model-call event for a single user, unaggregated -- the input the
/// task-cost report buckets by activation window and status-at-the-time,
/// which a `GROUP BY` sum can't do. Carries enough to both summarize (token
/// splits, models, duration) and list individually (the task detail
/// drill-down), so one query serves both.
#[derive(Debug, Clone)]
pub struct RawTelemetryEventSlice {
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_name: Option<String>,
    pub received_at: DateTime<Utc>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub duration_ms: u64,
    pub total_cost_usd_micros: u64,
    pub status: String,
    pub is_error: bool,
}

/// One stored event that carries no server price yet, with everything needed
/// to price it as of when it happened.
#[derive(Debug, Clone)]
pub struct RepriceCandidate {
    pub id: Uuid,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// The lane the call was billed under, carried through so repriced
    /// history lands on the same rate a freshly ingested call would.
    pub service_tier: Option<String>,
    pub reported_at: DateTime<Utc>,
    pub usage: conductor_domain::TokenUsage,
}

/// The outcome of pricing one candidate, ready to write back.
///
/// An unpriced outcome is still recorded: the reason is what makes unpriced
/// volume explainable rather than looking like an oversight.
#[derive(Debug, Clone)]
pub struct RepricedCost {
    pub id: Uuid,
    pub cost: PricedCost,
    pub catalog_version: Option<String>,
    pub basis: Option<PricingBasis>,
}

pub struct TelemetryRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl TelemetryRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    /// Persist a batch, recording Conductor's own cost alongside the client's.
    ///
    /// Pricing arrives already computed rather than being looked up here: the
    /// caller holds the in-memory rate table, so a batch costs no extra
    /// queries in the common case.
    pub async fn ingest(
        &self,
        project_id: Uuid,
        user: &User,
        installation_id: Uuid,
        events: &[PricedTelemetryEvent<'_>],
    ) -> Result<TelemetryBatchResponse, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let mut accepted = 0u32;
        let mut duplicates = 0u32;
        let received_at = Utc::now().to_rfc3339();
        let sub_role_ids =
            serde_json::to_string(&user.sub_role_ids).unwrap_or_else(|_| "[]".into());
        let tag_ids = serde_json::to_string(&user.tag_ids).unwrap_or_else(|_| "[]".into());

        for priced in events {
            let event = priced.event;
            let insert = match self.kind {
                DatabaseKind::Mysql => {
                    r#"
                INSERT INTO telemetry_events (
                    id, project_id, user_id, installation_id, request_id, session_id, event_type,
                    sequence, agent_name, provider, model, response_model, tokens_in, tokens_out,
                    cache_read_tokens, cache_write_tokens, reasoning_tokens,
                    tool_use_tokens, duration_ms,
                    status, error_category, reported_at,
                    received_at, service_tier,
                    server_cost_usd_micros, priced_catalog_version, pricing_basis,
                    unpriced_reason,
                    primary_role_snapshot, sub_role_ids_snapshot,
                    tag_ids_snapshot, tool_calls, active_agents
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
                ON DUPLICATE KEY UPDATE id = id
                "#
                }
                DatabaseKind::Sqlite | DatabaseKind::Postgres => {
                    r#"
                INSERT INTO telemetry_events (
                    id, project_id, user_id, installation_id, request_id, session_id, event_type,
                    sequence, agent_name, provider, model, response_model, tokens_in, tokens_out,
                    cache_read_tokens, cache_write_tokens, reasoning_tokens,
                    tool_use_tokens, duration_ms,
                    status, error_category, reported_at,
                    received_at, service_tier,
                    server_cost_usd_micros, priced_catalog_version, pricing_basis,
                    unpriced_reason,
                    primary_role_snapshot, sub_role_ids_snapshot,
                    tag_ids_snapshot, tool_calls, active_agents
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
                ON CONFLICT (id) DO NOTHING
                "#
                }
            };
            let inserted = sqlx::query(insert)
                .bind(event.event_id.to_string())
                .bind(project_id.to_string())
                .bind(user.id.to_string())
                .bind(installation_id.to_string())
                .bind(event.request_id.trim())
                .bind(event.session_id.as_deref())
                .bind(event.event_type.as_str())
                .bind(i64::from(event.sequence))
                .bind(event.agent_name.as_deref())
                .bind(event.provider.as_deref())
                .bind(event.model.as_deref())
                .bind(event.response_model.as_deref())
                .bind(to_i64(event.tokens_in))
                .bind(to_i64(event.tokens_out))
                .bind(to_i64(event.cache_read_tokens))
                .bind(to_i64(event.cache_write_tokens))
                .bind(to_i64(event.reasoning_tokens))
                .bind(to_i64(event.tool_use_tokens))
                .bind(to_i64(event.duration_ms))
                .bind(event.status.as_str())
                .bind(event.error_category.as_deref())
                .bind(event.reported_at.to_rfc3339())
                .bind(&received_at)
                .bind(event.service_tier.as_deref())
                .bind(priced.cost.cost_micros())
                .bind(priced.catalog_version.as_deref())
                .bind(priced.basis.map(PricingBasis::as_str))
                .bind(priced.cost.unpriced_reason().map(|reason| reason.as_str()))
                .bind(user.primary_role.as_str())
                .bind(&sub_role_ids)
                .bind(&tag_ids)
                .bind(if event.event_type == TelemetryEventType::ToolCall {
                    1i64
                } else {
                    0i64
                })
                .execute(&mut *tx)
                .await?;
            if inserted.rows_affected() == 0 {
                duplicates += 1;
                continue;
            }
            for resource in &event.resources {
                sqlx::query(
                    r#"
                    INSERT INTO telemetry_resource_attributions (
                        event_id, project_id, resource_id, version_id, relation,
                        plugin_installation_id
                    ) VALUES (?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(event.event_id.to_string())
                .bind(project_id.to_string())
                .bind(resource.resource_id.to_string())
                .bind(resource.version_id.to_string())
                .bind(resource.relation.as_str())
                .bind(resource.plugin_installation_id.as_deref())
                .execute(&mut *tx)
                .await?;
            }
            accepted += 1;
        }

        // Inside the same transaction as the events it accounts for. A
        // contact row that could commit without its events -- or events that
        // could commit without their contact row -- would record a state
        // that never existed, and coverage would be wrong in exactly the
        // situations it is meant to detect.
        //
        // Recorded even for a batch of nothing but duplicates: the client
        // did reach the server that day, which is the fact coverage asks
        // about. Only accepted events are counted, so the tally stays a
        // count of stored events rather than of delivery attempts.
        crate::repos::record_contact(
            &mut *tx,
            self.kind,
            installation_id,
            project_id,
            Utc::now(),
            accepted,
            0,
        )
        .await?;

        tx.commit().await?;
        Ok(TelemetryBatchResponse {
            accepted,
            duplicates,
            summary: None,
        })
    }

    pub async fn delivery_summary(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        installation_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        window_days: u16,
    ) -> Result<TelemetryDeliverySummary, sqlx::Error> {
        let project = self.kind.bind_parameter(1);
        let user = self.kind.bind_parameter(2);
        let installation = self.kind.bind_parameter(3);
        let from_marker = self.kind.bind_parameter(4);
        let to_marker = self.kind.bind_parameter(5);
        let sql = format!(
            r#"
            SELECT COUNT(*) AS events,
                   COUNT(DISTINCT e.request_id) AS requests,
                   COALESCE(SUM(CASE WHEN e.event_type = 'model_call' THEN 1 ELSE 0 END), 0) AS model_calls,
                   COALESCE(SUM(CASE WHEN e.event_type = 'tool_call' THEN 1 ELSE 0 END), 0) AS tool_calls,
                   COALESCE(SUM(e.tokens_in), 0) AS tokens_in,
                   COALESCE(SUM(e.tokens_out), 0) AS tokens_out,
                   COALESCE(SUM(e.cache_read_tokens), 0) AS cache_read_tokens,
                   COALESCE(SUM(e.server_cost_usd_micros), 0) AS estimated_cost_usd_micros,
                   COALESCE(SUM(CASE WHEN e.event_type = 'model_call' AND e.server_cost_usd_micros IS NULL THEN 1 ELSE 0 END), 0) AS unpriced_model_calls,
                   COALESCE(SUM(CASE WHEN EXISTS (
                       SELECT 1 FROM telemetry_resource_attributions a
                       WHERE a.event_id = e.id AND a.project_id = e.project_id
                   ) THEN 1 ELSE 0 END), 0) AS attributed_events,
                   COUNT(DISTINCT CASE WHEN EXISTS (
                       SELECT 1 FROM telemetry_resource_attributions a
                       WHERE a.event_id = e.id AND a.project_id = e.project_id
                   ) THEN e.request_id ELSE NULL END) AS attributed_requests,
                   COALESCE(SUM(CASE WHEN e.event_type = 'model_call' AND EXISTS (
                       SELECT 1 FROM telemetry_resource_attributions a
                       WHERE a.event_id = e.id AND a.project_id = e.project_id
                   ) THEN 1 ELSE 0 END), 0) AS attributed_model_calls,
                   COALESCE(SUM(CASE WHEN e.event_type = 'tool_call' AND EXISTS (
                       SELECT 1 FROM telemetry_resource_attributions a
                       WHERE a.event_id = e.id AND a.project_id = e.project_id
                   ) THEN 1 ELSE 0 END), 0) AS attributed_tool_calls,
                   COALESCE(SUM(CASE WHEN EXISTS (
                       SELECT 1 FROM telemetry_resource_attributions a
                       WHERE a.event_id = e.id AND a.project_id = e.project_id
                   ) THEN e.server_cost_usd_micros ELSE 0 END), 0) AS attributed_estimated_cost_usd_micros
            FROM telemetry_events e
            WHERE e.project_id = {project}
              AND e.user_id = {user}
              AND e.installation_id = {installation}
              AND e.received_at >= {from_marker}
              AND e.received_at <= {to_marker}
            "#
        );
        let row = sqlx::query(&sql)
            .bind(project_id.to_string())
            .bind(user_id.to_string())
            .bind(installation_id.to_string())
            .bind(from.to_rfc3339())
            .bind(to.to_rfc3339())
            .fetch_one(&self.pool)
            .await?;

        Ok(TelemetryDeliverySummary {
            installation_id,
            window_days,
            from,
            to,
            events: non_negative(row.get("events")),
            requests: non_negative(row.get("requests")),
            model_calls: non_negative(row.get("model_calls")),
            tool_calls: non_negative(row.get("tool_calls")),
            tokens_in: non_negative(row.get("tokens_in")),
            tokens_out: non_negative(row.get("tokens_out")),
            cache_read_tokens: non_negative(row.get("cache_read_tokens")),
            estimated_cost_usd_micros: non_negative(row.get("estimated_cost_usd_micros")),
            unpriced_model_calls: non_negative(row.get("unpriced_model_calls")),
            attributed_events: non_negative(row.get("attributed_events")),
            attributed_requests: non_negative(row.get("attributed_requests")),
            attributed_model_calls: non_negative(row.get("attributed_model_calls")),
            attributed_tool_calls: non_negative(row.get("attributed_tool_calls")),
            attributed_estimated_cost_usd_micros: non_negative(
                row.get("attributed_estimated_cost_usd_micros"),
            ),
        })
    }

    /// Windowed on `received_at`, like the project analytics and spend limits.
    ///
    /// `reported_at` is when EvoFlux says the work happened, which a client
    /// controls and can backdate by a whole outbox drain; `received_at` is when
    /// Conductor learned of it. Accounting figures use the second, so a closed
    /// period stays closed and a member's own page cannot disagree with the
    /// admin analytics for the same range. The event timestamps this returns
    /// stay on `reported_at` — those describe when the work ran.
    pub async fn usage_summary(
        &self,
        user_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<MemberUsageSummary, sqlx::Error> {
        let user = user_id.to_string();
        let from_value = from.to_rfc3339();
        let to_value = to.to_rfc3339();
        let row = sqlx::query(
            r#"
            SELECT COUNT(DISTINCT request_id) AS total_requests,
                   COALESCE(SUM(CASE WHEN event_type = ? THEN 1 ELSE 0 END), 0) AS model_calls,
                   COALESCE(SUM(CASE WHEN event_type = ? THEN 1 ELSE 0 END), 0) AS tool_calls,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS error_count,
                   COALESCE(SUM(tokens_in), 0) AS tokens_in,
                   COALESCE(SUM(tokens_out), 0) AS tokens_out,
                   COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                   COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                   COALESCE(SUM(server_cost_usd_micros), 0) AS cost_micros,
                   COALESCE(SUM(CASE WHEN event_type = 'model_call'
                                      AND server_cost_usd_micros IS NULL
                                 THEN 1 ELSE 0 END), 0) AS unpriced_model_calls
            FROM telemetry_events
            WHERE user_id = ? AND request_id IS NOT NULL
              AND received_at >= ? AND received_at <= ?
            "#,
        )
        .bind(TelemetryEventType::ModelCall.as_str())
        .bind(TelemetryEventType::ToolCall.as_str())
        .bind(TelemetryEventStatus::Error.as_str())
        .bind(&user)
        .bind(&from_value)
        .bind(&to_value)
        .fetch_one(&self.pool)
        .await?;

        let models = sqlx::query(
            r#"
            SELECT COALESCE(provider, ?) AS provider,
                   COALESCE(model, ?) AS model,
                   COUNT(*) AS calls,
                   COALESCE(SUM(tokens_in), 0) AS tokens_in,
                   COALESCE(SUM(tokens_out), 0) AS tokens_out,
                   COALESCE(SUM(server_cost_usd_micros), 0) AS cost_micros,
                   COALESCE(SUM(CASE WHEN server_cost_usd_micros IS NULL
                                 THEN 1 ELSE 0 END), 0) AS unpriced_calls
            FROM telemetry_events
            WHERE user_id = ? AND event_type = ?
              AND received_at >= ? AND received_at <= ?
            GROUP BY provider, model
            ORDER BY (COALESCE(SUM(tokens_in), 0) + COALESCE(SUM(tokens_out), 0)) DESC
            "#,
        )
        .bind(UNKNOWN_TELEMETRY_LABEL)
        .bind(UNKNOWN_TELEMETRY_LABEL)
        .bind(&user)
        .bind(TelemetryEventType::ModelCall.as_str())
        .bind(&from_value)
        .bind(&to_value)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            let tokens_in = non_negative(row.get::<i64, _>("tokens_in"));
            let tokens_out = non_negative(row.get::<i64, _>("tokens_out"));
            ModelUsageBreakdown {
                provider: row.get("provider"),
                model: row.get("model"),
                calls: non_negative(row.get("calls")),
                tokens_in,
                tokens_out,
                total_tokens: tokens_in.saturating_add(tokens_out),
                estimated_cost_usd_micros: non_negative(row.get::<i64, _>("cost_micros")),
                unpriced_calls: non_negative(row.get::<i64, _>("unpriced_calls")),
            }
        })
        .collect();

        let daily = sqlx::query(
            r#"
            SELECT SUBSTR(received_at, 1, 10) AS date,
                   COUNT(DISTINCT request_id) AS requests,
                   COALESCE(SUM(tokens_in), 0) AS tokens_in,
                   COALESCE(SUM(tokens_out), 0) AS tokens_out,
                   COALESCE(SUM(server_cost_usd_micros), 0) AS cost_micros
            FROM telemetry_events
            WHERE user_id = ? AND request_id IS NOT NULL
              AND received_at >= ? AND received_at <= ?
            GROUP BY SUBSTR(received_at, 1, 10)
            ORDER BY date
            "#,
        )
        .bind(&user)
        .bind(&from_value)
        .bind(&to_value)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            let tokens_in = non_negative(row.get::<i64, _>("tokens_in"));
            let tokens_out = non_negative(row.get::<i64, _>("tokens_out"));
            DailyTokenUsage {
                date: row.get("date"),
                requests: non_negative(row.get("requests")),
                tokens_in,
                tokens_out,
                total_tokens: tokens_in.saturating_add(tokens_out),
                estimated_cost_usd_micros: non_negative(row.get::<i64, _>("cost_micros")),
            }
        })
        .collect();

        let tokens_in = non_negative(row.get::<i64, _>("tokens_in"));
        let tokens_out = non_negative(row.get::<i64, _>("tokens_out"));
        Ok(MemberUsageSummary {
            from,
            to,
            total_requests: non_negative(row.get("total_requests")),
            model_calls: non_negative(row.get::<i64, _>("model_calls")),
            tool_calls: non_negative(row.get::<i64, _>("tool_calls")),
            error_count: non_negative(row.get::<i64, _>("error_count")),
            tokens_in,
            tokens_out,
            total_tokens: tokens_in.saturating_add(tokens_out),
            cache_read_tokens: non_negative(row.get::<i64, _>("cache_read_tokens")),
            reasoning_tokens: non_negative(row.get::<i64, _>("reasoning_tokens")),
            estimated_cost_usd_micros: non_negative(row.get::<i64, _>("cost_micros")),
            unpriced_model_calls: non_negative(row.get::<i64, _>("unpriced_model_calls")),
            models,
            daily,
        })
    }

    /// Raw per-model token and cost sums for the whole project, windowed on
    /// `received_at` like every other accounting view.
    ///
    /// Deliberately not scoped to governed/attributed activity: this is a
    /// spend report, and a model call that never touched a managed resource
    /// still costs money. Component-level cost (input vs output vs cache) is
    /// not computed here -- that needs the live rate table, which belongs to
    /// the server layer, not storage -- so the caller combines these sums
    /// with `conductor_domain::price_components`.
    pub async fn model_cost_totals(
        &self,
        project_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        filters: &CostReportFilters,
    ) -> Result<Vec<RawModelCostRow>, sqlx::Error> {
        let mut builder = QueryBuilder::<Any>::new("SELECT COALESCE(e.provider,");
        builder.push_bind(UNKNOWN_TELEMETRY_LABEL);
        builder.push(") AS provider, COALESCE(e.model,");
        builder.push_bind(UNKNOWN_TELEMETRY_LABEL);
        builder.push(
            r#") AS model,
               COUNT(*) AS calls,
               COALESCE(SUM(CASE WHEN e.server_cost_usd_micros IS NULL
                             THEN 1 ELSE 0 END), 0) AS unpriced_calls,
               COALESCE(SUM(e.tokens_in), 0) AS tokens_in,
               COALESCE(SUM(e.tokens_out), 0) AS tokens_out,
               COALESCE(SUM(e.cache_read_tokens), 0) AS cache_read_tokens,
               COALESCE(SUM(e.cache_write_tokens), 0) AS cache_write_tokens,
               COALESCE(SUM(e.reasoning_tokens), 0) AS reasoning_tokens,
               COALESCE(SUM(e.server_cost_usd_micros), 0) AS cost_micros
            FROM telemetry_events e
            WHERE e.project_id="#,
        );
        builder.push_bind(project_id.to_string());
        builder.push(" AND e.event_type=");
        builder.push_bind(TelemetryEventType::ModelCall.as_str());
        builder.push(" AND e.received_at>=");
        builder.push_bind(from.to_rfc3339());
        builder.push(" AND e.received_at<=");
        builder.push_bind(to.to_rfc3339());
        push_cost_report_filters(&mut builder, filters, "e");
        builder.push(" GROUP BY e.provider, e.model ORDER BY cost_micros DESC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| RawModelCostRow {
                provider: row.get("provider"),
                model: row.get("model"),
                calls: non_negative(row.get("calls")),
                unpriced_calls: non_negative(row.get("unpriced_calls")),
                tokens_in: non_negative(row.get::<i64, _>("tokens_in")),
                tokens_out: non_negative(row.get::<i64, _>("tokens_out")),
                cache_read_tokens: non_negative(row.get::<i64, _>("cache_read_tokens")),
                cache_write_tokens: non_negative(row.get::<i64, _>("cache_write_tokens")),
                reasoning_tokens: non_negative(row.get::<i64, _>("reasoning_tokens")),
                total_cost_usd_micros: non_negative(row.get::<i64, _>("cost_micros")),
            })
            .collect())
    }

    /// Raw per-member token and cost sums for the whole project, windowed and
    /// narrowed the same way as `model_cost_totals`. The member's current
    /// display name, email and role are joined in directly, since a cost
    /// report is read for people as they are today, not as they were at call
    /// time.
    pub async fn member_cost_totals(
        &self,
        project_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        filters: &CostReportFilters,
    ) -> Result<Vec<RawMemberCostRow>, sqlx::Error> {
        let mut builder = QueryBuilder::<Any>::new(
            r#"SELECT e.user_id, u.display_name, u.email, u.primary_role,
               COUNT(*) AS calls,
               COALESCE(SUM(CASE WHEN e.server_cost_usd_micros IS NULL
                             THEN 1 ELSE 0 END), 0) AS unpriced_calls,
               COALESCE(SUM(e.tokens_in), 0) AS tokens_in,
               COALESCE(SUM(e.tokens_out), 0) AS tokens_out,
               COALESCE(SUM(e.cache_read_tokens), 0) AS cache_read_tokens,
               COALESCE(SUM(e.cache_write_tokens), 0) AS cache_write_tokens,
               COALESCE(SUM(e.reasoning_tokens), 0) AS reasoning_tokens,
               COALESCE(SUM(e.server_cost_usd_micros), 0) AS cost_micros
            FROM telemetry_events e JOIN users u ON u.id=e.user_id
            WHERE e.project_id="#,
        );
        builder.push_bind(project_id.to_string());
        builder.push(" AND e.event_type=");
        builder.push_bind(TelemetryEventType::ModelCall.as_str());
        builder.push(" AND e.received_at>=");
        builder.push_bind(from.to_rfc3339());
        builder.push(" AND e.received_at<=");
        builder.push_bind(to.to_rfc3339());
        if let Some(role) = filters.primary_role {
            builder.push(" AND u.primary_role=");
            builder.push_bind(role.as_str());
        }
        push_cost_report_common_filters(&mut builder, filters, "e");
        builder.push(" GROUP BY e.user_id, u.display_name, u.email, u.primary_role ORDER BY cost_micros DESC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let user_id = Uuid::parse_str(&row.get::<String, _>("user_id")).ok()?;
                let primary_role =
                    PrimaryRole::parse(row.get::<String, _>("primary_role").as_str())?;
                Some(RawMemberCostRow {
                    user_id,
                    display_name: row.get("display_name"),
                    email: row.get("email"),
                    primary_role,
                    calls: non_negative(row.get("calls")),
                    unpriced_calls: non_negative(row.get("unpriced_calls")),
                    tokens_in: non_negative(row.get::<i64, _>("tokens_in")),
                    tokens_out: non_negative(row.get::<i64, _>("tokens_out")),
                    cache_read_tokens: non_negative(row.get::<i64, _>("cache_read_tokens")),
                    cache_write_tokens: non_negative(row.get::<i64, _>("cache_write_tokens")),
                    reasoning_tokens: non_negative(row.get::<i64, _>("reasoning_tokens")),
                    total_cost_usd_micros: non_negative(row.get::<i64, _>("cost_micros")),
                })
            })
            .collect())
    }

    /// Every model-call event for one user in the window, unaggregated and
    /// ordered by time -- the task-cost report's own building block for
    /// slicing a member's usage into activation windows and, inside those,
    /// by the Jira status active at each event's moment. Not filtered by
    /// role/tag/provider/model like the aggregate reports: a task's number
    /// must equal what `member_cost_totals` already showed for that member,
    /// or the two report views would silently disagree.
    pub async fn member_event_series(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<RawTelemetryEventSlice>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT request_id, session_id, agent_name, received_at, provider, model,
                      COALESCE(tokens_in,0) AS tokens_in,
                      COALESCE(tokens_out,0) AS tokens_out,
                      COALESCE(cache_read_tokens,0) AS cache_read_tokens,
                      COALESCE(cache_write_tokens,0) AS cache_write_tokens,
                      COALESCE(reasoning_tokens,0) AS reasoning_tokens,
                      COALESCE(duration_ms,0) AS duration_ms,
                      COALESCE(server_cost_usd_micros,0) AS cost_micros,
                      status
               FROM telemetry_events
               WHERE project_id = ? AND user_id = ? AND event_type = ?
                 AND received_at >= ? AND received_at <= ?
               ORDER BY received_at ASC"#,
        )
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(TelemetryEventType::ModelCall.as_str())
        .bind(from.to_rfc3339())
        .bind(to.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let received_at = row.get::<Option<String>, _>("received_at")?;
                let tokens_in = non_negative(row.get::<i64, _>("tokens_in"));
                let tokens_out = non_negative(row.get::<i64, _>("tokens_out"));
                let cache_read_tokens = non_negative(row.get::<i64, _>("cache_read_tokens"));
                let cache_write_tokens = non_negative(row.get::<i64, _>("cache_write_tokens"));
                let reasoning_tokens = non_negative(row.get::<i64, _>("reasoning_tokens"));
                let status = row.get::<String, _>("status");
                Some(RawTelemetryEventSlice {
                    request_id: row.get("request_id"),
                    session_id: row.get("session_id"),
                    agent_name: row.get("agent_name"),
                    received_at: parse_dt(received_at),
                    provider: row.get("provider"),
                    model: row.get("model"),
                    tokens_in,
                    tokens_out,
                    cache_read_tokens,
                    cache_write_tokens,
                    reasoning_tokens,
                    total_tokens: tokens_in
                        + tokens_out
                        + cache_read_tokens
                        + cache_write_tokens
                        + reasoning_tokens,
                    duration_ms: non_negative(row.get::<i64, _>("duration_ms")),
                    total_cost_usd_micros: non_negative(row.get::<i64, _>("cost_micros")),
                    is_error: status != TelemetryEventStatus::Success.as_str(),
                    status,
                })
            })
            .collect())
    }

    pub async fn activity(
        &self,
        user_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        limit: u32,
        offset: u32,
    ) -> Result<MemberActivityResponse, sqlx::Error> {
        let user = user_id.to_string();
        let from_value = from.to_rfc3339();
        let to_value = to.to_rfc3339();
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT request_id) FROM telemetry_events \
             WHERE user_id = ? AND request_id IS NOT NULL AND received_at >= ? AND received_at <= ?",
        )
        .bind(&user)
        .bind(&from_value)
        .bind(&to_value)
        .fetch_one(&self.pool)
        .await?;

        let rows = sqlx::query(
            r#"
            SELECT request_id, MAX(session_id) AS session_id,
                   MIN(reported_at) AS started_at, MAX(reported_at) AS finished_at,
                   MAX(provider) AS provider, MAX(model) AS model,
                   SUM(CASE WHEN event_type = ? THEN 1 ELSE 0 END) AS model_calls,
                   SUM(CASE WHEN event_type = ? THEN 1 ELSE 0 END) AS tool_calls,
                   COALESCE(SUM(tokens_in), 0) AS tokens_in,
                   COALESCE(SUM(tokens_out), 0) AS tokens_out,
                   COALESCE(
                       MAX(CASE WHEN event_type = 'request' THEN duration_ms END),
                       SUM(CASE WHEN event_type <> 'request' THEN duration_ms ELSE 0 END),
                       0
                   ) AS duration_ms,
                   COALESCE(SUM(server_cost_usd_micros), 0) AS cost_micros,
                   COALESCE(SUM(CASE WHEN event_type = ? AND server_cost_usd_micros IS NULL THEN 1 ELSE 0 END), 0) AS unpriced_model_calls,
                   SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS errors,
                   SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS blocked,
                   SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS cancelled
            FROM telemetry_events
            WHERE user_id = ? AND request_id IS NOT NULL
              AND received_at >= ? AND received_at <= ?
            GROUP BY request_id
            ORDER BY started_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(TelemetryEventType::ModelCall.as_str())
        .bind(TelemetryEventType::ToolCall.as_str())
        .bind(TelemetryEventType::ModelCall.as_str())
        .bind(TelemetryEventStatus::Error.as_str())
        .bind(TelemetryEventStatus::Blocked.as_str())
        .bind(TelemetryEventStatus::Cancelled.as_str())
        .bind(&user)
        .bind(&from_value)
        .bind(&to_value)
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(&self.pool)
        .await?;

        Ok(MemberActivityResponse {
            items: rows.into_iter().map(map_activity).collect(),
            total: non_negative(total),
            limit,
            offset,
        })
    }

    pub async fn request_detail(
        &self,
        user_id: Uuid,
        request_id: &str,
    ) -> Result<Option<MemberRequestDetail>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, request_id, session_id, event_type, sequence, agent_name,
                   provider, model, response_model, tokens_in, tokens_out, cache_read_tokens,
                   reasoning_tokens, tool_use_tokens, duration_ms, status, error_category,
                   server_cost_usd_micros AS estimated_cost_usd_micros,
                   reported_at
            FROM telemetry_events
            WHERE user_id = ? AND request_id = ?
            ORDER BY reported_at, sequence, id
            "#,
        )
        .bind(user_id.to_string())
        .bind(request_id)
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let attribution_rows = sqlx::query(
            r#"
            SELECT a.event_id, a.resource_id, a.version_id, a.relation,
                   r.kind, r.name, rv.version
            FROM telemetry_resource_attributions a
            JOIN telemetry_events e ON e.id = a.event_id
            JOIN resources r ON r.id = a.resource_id
            JOIN resource_versions rv ON rv.id = a.version_id
            WHERE e.user_id = ? AND e.request_id = ?
            ORDER BY r.kind, r.name, rv.version
            "#,
        )
        .bind(user_id.to_string())
        .bind(request_id)
        .fetch_all(&self.pool)
        .await?;
        let mut attributions: HashMap<String, Vec<TelemetryResourceAttributionDetail>> =
            HashMap::new();
        for row in attribution_rows {
            let Some(resource_id) =
                Uuid::parse_str(row.get::<String, _>("resource_id").as_str()).ok()
            else {
                continue;
            };
            let Some(version_id) =
                Uuid::parse_str(row.get::<String, _>("version_id").as_str()).ok()
            else {
                continue;
            };
            let Some(kind) = ResourceKind::parse(row.get::<String, _>("kind").as_str()) else {
                continue;
            };
            let Some(relation) =
                TelemetryResourceRelation::parse(row.get::<String, _>("relation").as_str())
            else {
                continue;
            };
            attributions.entry(row.get("event_id")).or_default().push(
                TelemetryResourceAttributionDetail {
                    resource_id,
                    version_id,
                    kind,
                    name: row.get("name"),
                    version: row.get("version"),
                    relation,
                },
            );
        }

        let started_at = parse_dt(rows[0].get("reported_at"));
        let finished_at = parse_dt(rows[rows.len() - 1].get("reported_at"));
        let mut model_calls = 0;
        let mut tool_calls = 0;
        let mut tokens_in = 0u64;
        let mut tokens_out = 0u64;
        let mut operation_duration_ms = 0u64;
        let mut request_duration_ms = None;
        let mut estimated_cost_usd_micros = 0u64;
        let mut unpriced_model_calls = 0u64;
        let mut request_status = TelemetryEventStatus::Success;
        let mut provider = None;
        let mut model = None;
        let session_id = rows[0].get("session_id");
        let mut events = Vec::with_capacity(rows.len());

        for row in rows {
            let event_id_value: String = row.get("id");
            let event_type = TelemetryEventType::parse(row.get::<String, _>("event_type").as_str())
                .unwrap_or(TelemetryEventType::ModelCall);
            let event_duration = non_negative(row.get::<i64, _>("duration_ms"));
            match event_type {
                TelemetryEventType::ModelCall => {
                    model_calls += 1;
                    provider = provider.or_else(|| row.get("provider"));
                    model = model.or_else(|| row.get("model"));
                    match row.get::<Option<i64>, _>("estimated_cost_usd_micros") {
                        Some(cost) => {
                            estimated_cost_usd_micros =
                                estimated_cost_usd_micros.saturating_add(non_negative(cost));
                        }
                        None => unpriced_model_calls = unpriced_model_calls.saturating_add(1),
                    }
                }
                TelemetryEventType::ToolCall => tool_calls += 1,
                TelemetryEventType::Request => request_duration_ms = Some(event_duration),
            }
            let event_tokens_in = non_negative(row.get::<i64, _>("tokens_in"));
            let event_tokens_out = non_negative(row.get::<i64, _>("tokens_out"));
            let status = TelemetryEventStatus::parse(row.get::<String, _>("status").as_str())
                .unwrap_or(TelemetryEventStatus::Error);
            request_status = merge_request_status(request_status, status);
            tokens_in = tokens_in.saturating_add(event_tokens_in);
            tokens_out = tokens_out.saturating_add(event_tokens_out);
            if event_type != TelemetryEventType::Request {
                operation_duration_ms = operation_duration_ms.saturating_add(event_duration);
            }
            events.push(TelemetryEventDetail {
                event_id: Uuid::parse_str(&event_id_value).unwrap_or_else(|_| Uuid::nil()),
                event_type,
                sequence: non_negative(row.get::<i64, _>("sequence")) as u32,
                agent_name: row.get("agent_name"),
                provider: row.get("provider"),
                model: row.get("model"),
                response_model: row.get("response_model"),
                tokens_in: event_tokens_in,
                tokens_out: event_tokens_out,
                cache_read_tokens: non_negative(row.get::<i64, _>("cache_read_tokens")),
                reasoning_tokens: non_negative(row.get::<i64, _>("reasoning_tokens")),
                tool_use_tokens: non_negative(row.get::<i64, _>("tool_use_tokens")),
                duration_ms: event_duration,
                status,
                error_category: row.get("error_category"),
                estimated_cost_usd_micros: row
                    .get::<Option<i64>, _>("estimated_cost_usd_micros")
                    .map(non_negative),
                resources: attributions.remove(&event_id_value).unwrap_or_default(),
                reported_at: parse_dt(row.get("reported_at")),
            });
        }

        Ok(Some(MemberRequestDetail {
            request: MemberActivityItem {
                request_id: request_id.to_string(),
                session_id,
                started_at,
                finished_at,
                provider,
                model,
                model_calls,
                tool_calls,
                tokens_in,
                tokens_out,
                total_tokens: tokens_in.saturating_add(tokens_out),
                duration_ms: request_duration_ms.unwrap_or(operation_duration_ms),
                estimated_cost_usd_micros,
                unpriced_model_calls,
                status: request_status,
            },
            events,
        }))
    }

    pub async fn unpriced_model_calls(
        &self,
        project_id: Uuid,
        after_id: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<RepriceCandidate>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, provider, model, service_tier, reported_at, tokens_in,
                   tokens_out, cache_read_tokens, cache_write_tokens,
                   reasoning_tokens
            FROM telemetry_events
            WHERE project_id = ?
              AND event_type = 'model_call'
              AND server_cost_usd_micros IS NULL
              AND (? IS NULL OR id > ?)
            ORDER BY id ASC
            LIMIT ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(after_id.map(|id| id.to_string()))
        .bind(after_id.map(|id| id.to_string()))
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let raw: String = row.get("id");
                let id = Uuid::parse_str(&raw).ok()?;
                let reported_at: String = row.get("reported_at");
                Some(RepriceCandidate {
                    id,
                    provider: row.get("provider"),
                    model: row.get("model"),
                    service_tier: row.get("service_tier"),
                    reported_at: parse_dt(reported_at),
                    usage: conductor_domain::TokenUsage {
                        tokens_in: row.get("tokens_in"),
                        tokens_out: row.get("tokens_out"),
                        cache_read: row.get("cache_read_tokens"),
                        cache_write: row.get("cache_write_tokens"),
                        reasoning: row.get("reasoning_tokens"),
                    },
                })
            })
            .collect())
    }

    /// Write back a batch of repriced rows in one transaction.
    ///
    /// Guarded by `server_cost_usd_micros IS NULL` so a concurrent ingest that
    /// priced the row first wins, and a re-run cannot double-apply.
    pub async fn apply_repriced_costs(&self, priced: &[RepricedCost]) -> Result<u32, sqlx::Error> {
        if priced.is_empty() {
            return Ok(0);
        }
        let mut updated = 0u32;
        let mut tx = self.pool.begin().await?;
        for row in priced {
            let affected = sqlx::query(
                r#"
                UPDATE telemetry_events
                SET server_cost_usd_micros = ?,
                    priced_catalog_version = ?,
                    pricing_basis = ?,
                    unpriced_reason = ?
                WHERE id = ? AND server_cost_usd_micros IS NULL
                "#,
            )
            .bind(row.cost.cost_micros())
            .bind(row.catalog_version.as_deref())
            .bind(row.basis.map(PricingBasis::as_str))
            .bind(row.cost.unpriced_reason().map(|reason| reason.as_str()))
            .bind(row.id.to_string())
            .execute(&mut *tx)
            .await?
            .rows_affected();
            updated += u32::from(affected > 0);
        }
        tx.commit().await?;
        Ok(updated)
    }
}

fn map_activity(row: sqlx::any::AnyRow) -> MemberActivityItem {
    let tokens_in = non_negative(row.get::<i64, _>("tokens_in"));
    let tokens_out = non_negative(row.get::<i64, _>("tokens_out"));
    let errors = non_negative(row.get::<i64, _>("errors"));
    let blocked = non_negative(row.get::<i64, _>("blocked"));
    let cancelled = non_negative(row.get::<i64, _>("cancelled"));
    MemberActivityItem {
        request_id: row.get("request_id"),
        session_id: row.get("session_id"),
        started_at: parse_dt(row.get("started_at")),
        finished_at: parse_dt(row.get("finished_at")),
        provider: row.get("provider"),
        model: row.get("model"),
        model_calls: non_negative(row.get::<i64, _>("model_calls")),
        tool_calls: non_negative(row.get::<i64, _>("tool_calls")),
        tokens_in,
        tokens_out,
        total_tokens: tokens_in.saturating_add(tokens_out),
        duration_ms: non_negative(row.get::<i64, _>("duration_ms")),
        estimated_cost_usd_micros: non_negative(row.get::<i64, _>("cost_micros")),
        unpriced_model_calls: non_negative(row.get::<i64, _>("unpriced_model_calls")),
        status: if errors > 0 {
            TelemetryEventStatus::Error
        } else if blocked > 0 {
            TelemetryEventStatus::Blocked
        } else if cancelled > 0 {
            TelemetryEventStatus::Cancelled
        } else {
            TelemetryEventStatus::Success
        },
    }
}

fn merge_request_status(
    current: TelemetryEventStatus,
    event: TelemetryEventStatus,
) -> TelemetryEventStatus {
    match (current, event) {
        (TelemetryEventStatus::Error, _) | (_, TelemetryEventStatus::Error) => {
            TelemetryEventStatus::Error
        }
        (TelemetryEventStatus::Blocked, _) | (_, TelemetryEventStatus::Blocked) => {
            TelemetryEventStatus::Blocked
        }
        (TelemetryEventStatus::Cancelled, _) | (_, TelemetryEventStatus::Cancelled) => {
            TelemetryEventStatus::Cancelled
        }
        _ => TelemetryEventStatus::Success,
    }
}

fn to_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

fn non_negative(value: i64) -> u64 {
    value.max(0) as u64
}

/// Provider, model and tag narrowing, shared by both cost reports.
///
/// Role and tag both resolve against the member as they are **today**, not
/// against the role and tags snapshotted onto the event. A cost report is
/// read to answer "what is this team spending", and a member who changed
/// role last week should appear under the role they hold now rather than
/// splitting across both.
fn push_cost_report_common_filters(
    builder: &mut QueryBuilder<'_, Any>,
    filters: &CostReportFilters,
    alias: &str,
) {
    if let Some(provider) = filters.provider.as_deref() {
        builder.push(format!(" AND {alias}.provider="));
        builder.push_bind(provider.to_string());
    }
    if let Some(model) = filters.model.as_deref() {
        builder.push(format!(" AND {alias}.model="));
        builder.push_bind(model.to_string());
    }
    if let Some(tag_id) = filters.tag_id {
        builder.push(format!(
            " AND EXISTS (SELECT 1 FROM user_tags ut              WHERE ut.user_id={alias}.user_id AND ut.tag_id="
        ));
        builder.push_bind(tag_id.to_string());
        builder.push(")");
    }
}

/// The same narrowing plus role, for a query that does not join `users`
/// itself. The member report joins it and pushes the role clause inline.
fn push_cost_report_filters(
    builder: &mut QueryBuilder<'_, Any>,
    filters: &CostReportFilters,
    alias: &str,
) {
    if let Some(role) = filters.primary_role {
        builder.push(format!(
            " AND EXISTS (SELECT 1 FROM users u WHERE u.id={alias}.user_id              AND u.primary_role="
        ));
        builder.push_bind(role.as_str());
        builder.push(")");
    }
    push_cost_report_common_filters(builder, filters, alias);
}
