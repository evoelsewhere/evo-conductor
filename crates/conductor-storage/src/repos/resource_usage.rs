use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use conductor_domain::{
    PrimaryRole, ResourceInventoryObservedState, ResourceKind, ResourceUsageActivityItem,
    ResourceUsageAnalytics, ResourceUsageBreakdown, ResourceUsageDay, ResourceUsageMember,
    ResourceUsageModel, ResourceUsageRole, ResourceUsageScope, ResourceUsageTool,
    ResourceUsageTotals, TelemetryEventStatus, TelemetryResourceRelation, TelemetryToolCategory,
    UNKNOWN_TELEMETRY_LABEL,
};
use sqlx::{Any, Pool, QueryBuilder, Row};
use uuid::Uuid;

use crate::core::dialect::DatabaseKind;
use crate::core::mapping::parse_dt;

#[derive(Debug, Clone)]
pub struct ResourceUsageQuery {
    pub project_id: Uuid,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub primary_role: Option<PrimaryRole>,
    pub resource_kind: Option<ResourceKind>,
    pub resource_id: Option<Uuid>,
    pub version_id: Option<Uuid>,
    pub status: Option<TelemetryEventStatus>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub installation_id: Option<Uuid>,
    pub relation: Option<TelemetryResourceRelation>,
    pub tool_name: Option<String>,
    pub scope: ResourceUsageScope,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone)]
pub struct ResourceUsageRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl ResourceUsageRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    pub async fn analytics(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<ResourceUsageAnalytics, sqlx::Error> {
        // These panels are independent read models. Running them concurrently
        // keeps a rich dashboard from paying the sum of eight query latencies,
        // while the pool still provides a hard concurrency bound per process.
        let resources = async {
            if query.scope == ResourceUsageScope::Governed {
                self.resources(query).await
            } else {
                Ok(Vec::new())
            }
        };
        let activity_page = async {
            if query.scope == ResourceUsageScope::Governed {
                self.activity(query).await
            } else {
                Ok((Vec::new(), 0))
            }
        };
        let (totals, daily, resources, members, models, roles, tools, activity_page) = tokio::try_join!(
            self.totals(query),
            self.daily(query),
            resources,
            self.members(query),
            self.models(query),
            self.roles(query),
            self.tools(query),
            activity_page,
        )?;
        let (activity, activity_total) = activity_page;
        Ok(ResourceUsageAnalytics {
            from: query.from,
            to: query.to,
            scope: query.scope,
            totals,
            daily,
            resources,
            members,
            models,
            roles,
            tools,
            activity,
            activity_total,
            limit: query.limit,
            offset: query.offset,
        })
    }

    async fn totals(&self, query: &ResourceUsageQuery) -> Result<ResourceUsageTotals, sqlx::Error> {
        let request_identity = request_identity_sql(self.kind, "e");
        let mut builder = QueryBuilder::<Any>::new(format!(
            r#"SELECT COUNT(DISTINCT ({request_identity})) AS requests,
              COALESCE(SUM(CASE WHEN e.event_type='model_call' THEN 1 ELSE 0 END),0) AS model_calls,
              COALESCE(SUM(CASE WHEN e.event_type='tool_call' THEN 1 ELSE 0 END),0) AS tool_calls,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='success' THEN {request_identity} END) AS successes,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='error' THEN {request_identity} END) AS errors,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='blocked' THEN {request_identity} END) AS blocked,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='cancelled' THEN {request_identity} END) AS cancelled,
              COALESCE(SUM(e.tokens_in),0) AS tokens_in,
              COALESCE(SUM(e.tokens_out),0) AS tokens_out,
              COALESCE(SUM(e.cache_read_tokens),0) AS cache_read_tokens,
              COALESCE(SUM(e.reasoning_tokens),0) AS reasoning_tokens,
              COALESCE(SUM(e.tool_use_tokens),0) AS tool_use_tokens,
              COALESCE(SUM(e.estimated_cost_usd_micros),0) AS cost_micros,
              COALESCE(SUM(CASE WHEN e.event_type='model_call' AND e.estimated_cost_usd_micros IS NULL THEN 1 ELSE 0 END),0) AS unpriced_model_calls,
              COALESCE(SUM(CASE WHEN e.event_type='request' THEN e.duration_ms ELSE 0 END),0) AS duration_ms "#,
        ));
        push_scoped_events(&mut builder, query);
        let row = builder.build().fetch_one(&self.pool).await?;
        let requests = n(row.get("requests"));
        let tokens_in = n(row.get("tokens_in"));
        let tokens_out = n(row.get("tokens_out"));
        let cache_read_tokens = n(row.get("cache_read_tokens"));
        let reasoning_tokens = n(row.get("reasoning_tokens"));
        let tool_use_tokens = n(row.get("tool_use_tokens"));
        // Cache-read, reasoning and tool-use counters are provider breakdowns
        // already included in input/output totals. Adding them again inflates
        // usage and makes project analytics disagree with member receipts.
        let total_tokens = tokens_in.saturating_add(tokens_out);
        let duration_ms = n(row.get("duration_ms"));
        let comparison_scope = match query.scope {
            ResourceUsageScope::All => ResourceUsageScope::Governed,
            ResourceUsageScope::Governed => ResourceUsageScope::All,
        };
        let (inventory, resource_uses, comparison_requests) = tokio::try_join!(
            self.inventory_totals(query),
            self.resource_use_count(query),
            self.request_count(query, comparison_scope),
        )?;
        let (all_requests, governed_requests) = match query.scope {
            ResourceUsageScope::All => (requests, comparison_requests),
            ResourceUsageScope::Governed => (comparison_requests, requests),
        };
        Ok(ResourceUsageTotals {
            reported_installations: inventory.reported_installations,
            installed_installations: inventory.installed_installations,
            installed_members: inventory.installed_members,
            pending_installations: inventory.pending_installations,
            attention_installations: inventory.attention_installations,
            all_requests,
            governed_requests,
            requests,
            resource_uses,
            model_calls: n(row.get("model_calls")),
            tool_calls: n(row.get("tool_calls")),
            successes: n(row.get("successes")),
            errors: n(row.get("errors")),
            blocked: n(row.get("blocked")),
            cancelled: n(row.get("cancelled")),
            tokens_in,
            tokens_out,
            cache_read_tokens,
            reasoning_tokens,
            tool_use_tokens,
            total_tokens,
            estimated_cost_usd_micros: n(row.get("cost_micros")),
            unpriced_model_calls: n(row.get("unpriced_model_calls")),
            average_tokens_per_request: total_tokens.checked_div(requests).unwrap_or_default(),
            average_duration_ms: duration_ms.checked_div(requests).unwrap_or_default(),
        })
    }

    async fn request_count(
        &self,
        query: &ResourceUsageQuery,
        scope: ResourceUsageScope,
    ) -> Result<u64, sqlx::Error> {
        let request_identity = request_identity_sql(self.kind, "e");
        let mut builder = QueryBuilder::<Any>::new(format!(
            "SELECT COUNT(DISTINCT ({request_identity})) AS requests"
        ));
        push_events_for_scope(&mut builder, query, scope);
        Ok(n(builder
            .build()
            .fetch_one(&self.pool)
            .await?
            .get("requests")))
    }

    async fn resource_use_count(&self, query: &ResourceUsageQuery) -> Result<u64, sqlx::Error> {
        let mut builder = QueryBuilder::<Any>::new(
            "SELECT COUNT(*) AS count FROM (SELECT e.user_id,e.request_id,a.resource_id,a.version_id,a.relation ",
        );
        push_filtered_from(&mut builder, query);
        builder.push(
            " GROUP BY e.user_id,e.request_id,a.resource_id,a.version_id,a.relation) resource_uses",
        );
        Ok(n(builder.build().fetch_one(&self.pool).await?.get("count")))
    }

    async fn daily(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<Vec<ResourceUsageDay>, sqlx::Error> {
        let request_identity = request_identity_sql(self.kind, "e");
        let mut builder = QueryBuilder::<Any>::new(format!(
            r#"SELECT SUBSTR(e.received_at,1,10) AS date,
              COUNT(DISTINCT ({request_identity})) AS requests,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='success' THEN {request_identity} END) AS successes,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='error' THEN {request_identity} END) AS errors,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='blocked' THEN {request_identity} END) AS blocked,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='cancelled' THEN {request_identity} END) AS cancelled,
              COALESCE(SUM(e.tokens_in),0) AS tokens_in,COALESCE(SUM(e.tokens_out),0) AS tokens_out,
              COALESCE(SUM(e.cache_read_tokens),0) AS cache_read_tokens,
              COALESCE(SUM(e.reasoning_tokens),0) AS reasoning_tokens,
              COALESCE(SUM(e.tool_use_tokens),0) AS tool_use_tokens,
              COALESCE(SUM(e.estimated_cost_usd_micros),0) AS cost_micros,
              COALESCE(SUM(CASE WHEN e.event_type='model_call' AND e.estimated_cost_usd_micros IS NULL THEN 1 ELSE 0 END),0) AS unpriced_model_calls "#,
        ));
        push_scoped_events(&mut builder, query);
        builder.push(" GROUP BY SUBSTR(e.received_at,1,10) ORDER BY date");
        Ok(builder
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| ResourceUsageDay {
                date: row.get("date"),
                requests: n(row.get("requests")),
                successes: n(row.get("successes")),
                errors: n(row.get("errors")),
                blocked: n(row.get("blocked")),
                cancelled: n(row.get("cancelled")),
                tokens_in: n(row.get("tokens_in")),
                tokens_out: n(row.get("tokens_out")),
                cache_read_tokens: n(row.get("cache_read_tokens")),
                reasoning_tokens: n(row.get("reasoning_tokens")),
                tool_use_tokens: n(row.get("tool_use_tokens")),
                estimated_cost_usd_micros: n(row.get("cost_micros")),
                unpriced_model_calls: n(row.get("unpriced_model_calls")),
            })
            .collect())
    }

    async fn resources(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<Vec<ResourceUsageBreakdown>, sqlx::Error> {
        let request_identity = request_identity_sql(self.kind, "e");
        let mut builder = QueryBuilder::<Any>::new(format!(
            r#"SELECT a.resource_id,a.version_id,r.kind,r.name,rv.version,a.relation,
              COUNT(DISTINCT ({request_identity})) AS uses,COUNT(DISTINCT e.user_id) AS members,
              COUNT(DISTINCT ({request_identity})) AS requests,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='success' THEN {request_identity} END) AS successes,
              COUNT(DISTINCT CASE WHEN e.event_type='request' AND e.status='error' THEN {request_identity} END) AS errors,
              COALESCE(SUM(CASE WHEN e.event_type='model_call' THEN 1 ELSE 0 END),0) AS model_calls,
              COALESCE(SUM(CASE WHEN e.event_type='tool_call' THEN 1 ELSE 0 END),0) AS tool_calls,
              COALESCE(SUM(e.tokens_in+e.tokens_out),0) AS total_tokens,
              COALESCE(SUM(e.estimated_cost_usd_micros),0) AS cost_micros,
              MAX(e.received_at) AS last_used_at "#,
        ));
        push_filtered_from(&mut builder, query);
        builder.push(" GROUP BY a.resource_id,a.version_id,r.kind,r.name,rv.version,a.relation ORDER BY uses DESC,r.name LIMIT 25");
        Ok(builder
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| {
                Some(ResourceUsageBreakdown {
                    resource_id: uuid(row.get("resource_id"))?,
                    version_id: uuid(row.get("version_id"))?,
                    kind: ResourceKind::parse(row.get::<String, _>("kind").as_str())?,
                    name: row.get("name"),
                    version: row.get("version"),
                    relation: TelemetryResourceRelation::parse(
                        row.get::<String, _>("relation").as_str(),
                    )?,
                    uses: n(row.get("uses")),
                    members: n(row.get("members")),
                    requests: n(row.get("requests")),
                    successes: n(row.get("successes")),
                    errors: n(row.get("errors")),
                    model_calls: n(row.get("model_calls")),
                    tool_calls: n(row.get("tool_calls")),
                    total_tokens: n(row.get("total_tokens")),
                    estimated_cost_usd_micros: n(row.get("cost_micros")),
                    last_used_at: parse_dt(row.get("last_used_at")),
                })
            })
            .collect())
    }

    async fn members(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<Vec<ResourceUsageMember>, sqlx::Error> {
        let resource_uses = self.member_resource_uses(query).await?;
        let request_identity = request_identity_sql(self.kind, "e");
        let mut builder = QueryBuilder::<Any>::new(format!(
            r#"SELECT e.user_id,u.display_name,u.email,u.primary_role,
              COUNT(DISTINCT ({request_identity})) AS requests,
              COALESCE(SUM(CASE WHEN e.event_type='model_call' THEN 1 ELSE 0 END),0) AS model_calls,
              COALESCE(SUM(CASE WHEN e.event_type='tool_call' THEN 1 ELSE 0 END),0) AS tool_calls,
              COUNT(DISTINCT e.installation_id) AS installations,
              COALESCE(SUM(e.tokens_in+e.tokens_out),0) AS total_tokens,
              COALESCE(SUM(e.estimated_cost_usd_micros),0) AS cost_micros,
              MAX(e.received_at) AS last_received_at "#,
        ));
        push_scoped_events(&mut builder, query);
        builder.push(" GROUP BY e.user_id,u.display_name,u.email,u.primary_role ORDER BY requests DESC,u.display_name LIMIT 25");
        Ok(builder
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| {
                let requests = n(row.get("requests"));
                let user_id = uuid(row.get("user_id"))?;
                Some(ResourceUsageMember {
                    user_id,
                    display_name: row.get("display_name"),
                    email: row.get("email"),
                    primary_role: PrimaryRole::parse(
                        row.get::<String, _>("primary_role").as_str(),
                    )?,
                    requests,
                    resource_uses: resource_uses.get(&user_id).copied().unwrap_or_default(),
                    model_calls: n(row.get("model_calls")),
                    tool_calls: n(row.get("tool_calls")),
                    installations: n(row.get("installations")),
                    total_tokens: n(row.get("total_tokens")),
                    estimated_cost_usd_micros: n(row.get("cost_micros")),
                    last_received_at: parse_dt(row.get("last_received_at")),
                })
            })
            .collect())
    }

    async fn models(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<Vec<ResourceUsageModel>, sqlx::Error> {
        let mut builder = QueryBuilder::<Any>::new("SELECT COALESCE(e.provider,");
        builder.push_bind(UNKNOWN_TELEMETRY_LABEL);
        builder.push(") AS provider,COALESCE(e.model,");
        builder.push_bind(UNKNOWN_TELEMETRY_LABEL);
        builder.push(r#") AS model,COUNT(*) AS calls,COALESCE(SUM(e.tokens_in+e.tokens_out),0) AS total_tokens,
          COALESCE(SUM(e.estimated_cost_usd_micros),0) AS cost_micros,
          COALESCE(SUM(CASE WHEN e.estimated_cost_usd_micros IS NULL THEN 1 ELSE 0 END),0) AS unpriced_calls "#);
        push_scoped_events(&mut builder, query);
        builder.push(" AND e.event_type='model_call' GROUP BY e.provider,e.model ORDER BY calls DESC LIMIT 20");
        Ok(builder
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| ResourceUsageModel {
                provider: row.get("provider"),
                model: row.get("model"),
                calls: n(row.get("calls")),
                total_tokens: n(row.get("total_tokens")),
                estimated_cost_usd_micros: n(row.get("cost_micros")),
                unpriced_calls: n(row.get("unpriced_calls")),
            })
            .collect())
    }

    async fn roles(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<Vec<ResourceUsageRole>, sqlx::Error> {
        let request_identity = request_identity_sql(self.kind, "e");
        let mut builder = QueryBuilder::<Any>::new(format!(
            r#"SELECT COALESCE(e.primary_role_snapshot,u.primary_role) AS primary_role,
              COUNT(DISTINCT ({request_identity})) AS requests,
              COALESCE(SUM(CASE WHEN e.event_type='model_call' THEN 1 ELSE 0 END),0) AS model_calls,
              COALESCE(SUM(CASE WHEN e.event_type='tool_call' THEN 1 ELSE 0 END),0) AS tool_calls,
              COALESCE(SUM(e.tokens_in+e.tokens_out),0) AS total_tokens,
              COALESCE(SUM(e.estimated_cost_usd_micros),0) AS cost_micros "#,
        ));
        push_scoped_events(&mut builder, query);
        builder.push(
            " GROUP BY COALESCE(e.primary_role_snapshot,u.primary_role) ORDER BY requests DESC",
        );
        Ok(builder
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| {
                Some(ResourceUsageRole {
                    primary_role: PrimaryRole::parse(
                        row.get::<String, _>("primary_role").as_str(),
                    )?,
                    requests: n(row.get("requests")),
                    model_calls: n(row.get("model_calls")),
                    tool_calls: n(row.get("tool_calls")),
                    total_tokens: n(row.get("total_tokens")),
                    estimated_cost_usd_micros: n(row.get("cost_micros")),
                })
            })
            .collect())
    }

    async fn tools(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<Vec<ResourceUsageTool>, sqlx::Error> {
        let mut builder = QueryBuilder::<Any>::new("SELECT COALESCE(e.tool_name,");
        builder.push_bind(UNKNOWN_TELEMETRY_LABEL);
        builder.push(") AS tool_name,COALESCE(e.tool_category,");
        builder.push_bind(TelemetryToolCategory::Other.as_str());
        builder.push(
            r#") AS category,COUNT(*) AS calls,
              COALESCE(SUM(CASE WHEN e.status='success' THEN 1 ELSE 0 END),0) AS successes,
              COALESCE(SUM(CASE WHEN e.status='error' THEN 1 ELSE 0 END),0) AS errors,
              COALESCE(SUM(CASE WHEN e.status='blocked' THEN 1 ELSE 0 END),0) AS blocked,
              COALESCE(SUM(CASE WHEN e.status='cancelled' THEN 1 ELSE 0 END),0) AS cancelled,
              CAST(COALESCE(AVG(e.duration_ms),0) AS BIGINT) AS average_duration_ms,
              MAX(e.received_at) AS last_used_at "#,
        );
        push_scoped_events(&mut builder, query);
        builder.push(" AND e.event_type='tool_call' GROUP BY e.tool_name,e.tool_category ORDER BY calls DESC LIMIT 25");
        Ok(builder
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| {
                Some(ResourceUsageTool {
                    tool_name: row.get("tool_name"),
                    category: TelemetryToolCategory::parse(
                        row.get::<String, _>("category").as_str(),
                    )?,
                    calls: n(row.get("calls")),
                    successes: n(row.get("successes")),
                    errors: n(row.get("errors")),
                    blocked: n(row.get("blocked")),
                    cancelled: n(row.get("cancelled")),
                    average_duration_ms: n(row.get("average_duration_ms")),
                    last_used_at: parse_dt(row.get("last_used_at")),
                })
            })
            .collect())
    }

    async fn inventory_totals(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<InventoryTotals, sqlx::Error> {
        let mut builder = QueryBuilder::<Any>::new(
            "SELECT i.installation_id,c.user_id,i.observed_state FROM installation_resource_inventory i JOIN client_installations c ON c.id=i.installation_id JOIN users u ON u.id=c.user_id JOIN resources r ON r.id=i.resource_id WHERE i.project_id=",
        );
        builder.push_bind(query.project_id.to_string());
        builder.push(" AND i.observed_state<>");
        builder.push_bind(ResourceInventoryObservedState::Removed.as_str());
        push_inventory_filters(&mut builder, query);
        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut totals = InventoryTotals::default();
        let mut installed_installations = HashSet::new();
        let mut installed_members = HashSet::new();
        for row in rows {
            totals.reported_installations += 1;
            let state: String = row.get("observed_state");
            if ResourceInventoryObservedState::INSTALLED
                .iter()
                .any(|candidate| candidate.as_str() == state)
            {
                installed_installations.insert(row.get::<String, _>("installation_id"));
                installed_members.insert(row.get::<String, _>("user_id"));
            } else if ResourceInventoryObservedState::PENDING
                .iter()
                .any(|candidate| candidate.as_str() == state)
            {
                totals.pending_installations += 1;
            } else {
                totals.attention_installations += 1;
            }
        }
        totals.installed_installations = installed_installations.len() as u64;
        totals.installed_members = installed_members.len() as u64;
        Ok(totals)
    }

    async fn member_resource_uses(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<HashMap<Uuid, u64>, sqlx::Error> {
        let mut builder = QueryBuilder::<Any>::new(
            "SELECT user_id,COUNT(*) AS uses FROM (SELECT e.user_id,e.request_id,a.resource_id,a.version_id,a.relation ",
        );
        push_filtered_from(&mut builder, query);
        builder.push(
            " GROUP BY e.user_id,e.request_id,a.resource_id,a.version_id,a.relation) member_uses GROUP BY user_id",
        );
        Ok(builder
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| Some((uuid(row.get("user_id"))?, n(row.get("uses")))))
            .collect())
    }

    async fn activity(
        &self,
        query: &ResourceUsageQuery,
    ) -> Result<(Vec<ResourceUsageActivityItem>, u64), sqlx::Error> {
        let mut count = QueryBuilder::<Any>::new("SELECT COUNT(*) AS count FROM (SELECT e.request_id,e.user_id,a.resource_id,a.version_id,a.relation ");
        push_filtered_from(&mut count, query);
        count.push(" GROUP BY e.request_id,e.user_id,a.resource_id,a.version_id,a.relation) rows");
        let total = n(count.build().fetch_one(&self.pool).await?.get("count"));
        let mut builder = QueryBuilder::<Any>::new(
            r#"SELECT e.request_id,e.user_id,u.display_name,COALESCE(MAX(e.primary_role_snapshot),u.primary_role) AS primary_role,
              a.resource_id,a.version_id,r.kind,r.name AS resource_name,rv.version,a.relation,MAX(e.received_at) AS occurred_at,
              COALESCE(SUM(CASE WHEN e.event_type='request' AND e.status='error' THEN 1 ELSE 0 END),0) AS errors,
              COALESCE(SUM(CASE WHEN e.event_type='request' AND e.status='blocked' THEN 1 ELSE 0 END),0) AS blocked,
              COALESCE(SUM(CASE WHEN e.event_type='request' AND e.status='cancelled' THEN 1 ELSE 0 END),0) AS cancelled,
              MAX(e.provider) AS provider,MAX(e.model) AS model,
              COALESCE(SUM(CASE WHEN e.event_type='model_call' THEN 1 ELSE 0 END),0) AS model_calls,
              COALESCE(SUM(CASE WHEN e.event_type='tool_call' THEN 1 ELSE 0 END),0) AS tool_calls,
              COALESCE(SUM(e.tokens_in+e.tokens_out),0) AS total_tokens,
              COALESCE(SUM(e.estimated_cost_usd_micros),0) AS cost_micros,
              COALESCE(SUM(CASE WHEN e.event_type='model_call' AND e.estimated_cost_usd_micros IS NULL THEN 1 ELSE 0 END),0) AS unpriced_model_calls,
              COALESCE(MAX(CASE WHEN e.event_type='request' THEN e.duration_ms END),
                       SUM(CASE WHEN e.event_type<>'request' THEN e.duration_ms ELSE 0 END),0) AS duration_ms "#,
        );
        push_filtered_from(&mut builder, query);
        builder.push(" GROUP BY e.request_id,e.user_id,u.display_name,u.primary_role,a.resource_id,a.version_id,r.kind,r.name,rv.version,a.relation ORDER BY occurred_at DESC LIMIT ");
        builder.push_bind(i64::from(query.limit));
        builder.push(" OFFSET ");
        builder.push_bind(i64::from(query.offset));
        let items = builder
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| {
                let errors = n(row.get("errors"));
                let blocked = n(row.get("blocked"));
                let cancelled = n(row.get("cancelled"));
                Some(ResourceUsageActivityItem {
                    request_id: row.get("request_id"),
                    user_id: uuid(row.get("user_id"))?,
                    display_name: row.get("display_name"),
                    primary_role: PrimaryRole::parse(
                        row.get::<String, _>("primary_role").as_str(),
                    )?,
                    resource_id: uuid(row.get("resource_id"))?,
                    version_id: uuid(row.get("version_id"))?,
                    kind: ResourceKind::parse(row.get::<String, _>("kind").as_str())?,
                    resource_name: row.get("resource_name"),
                    version: row.get("version"),
                    relation: TelemetryResourceRelation::parse(
                        row.get::<String, _>("relation").as_str(),
                    )?,
                    occurred_at: parse_dt(row.get("occurred_at")),
                    status: if errors > 0 {
                        TelemetryEventStatus::Error
                    } else if blocked > 0 {
                        TelemetryEventStatus::Blocked
                    } else if cancelled > 0 {
                        TelemetryEventStatus::Cancelled
                    } else {
                        TelemetryEventStatus::Success
                    },
                    provider: row.get("provider"),
                    model: row.get("model"),
                    model_calls: n(row.get("model_calls")),
                    tool_calls: n(row.get("tool_calls")),
                    total_tokens: n(row.get("total_tokens")),
                    estimated_cost_usd_micros: n(row.get("cost_micros")),
                    unpriced_model_calls: n(row.get("unpriced_model_calls")),
                    duration_ms: n(row.get("duration_ms")),
                })
            })
            .collect();
        Ok((items, total))
    }
}

#[derive(Debug, Default)]
struct InventoryTotals {
    reported_installations: u64,
    installed_installations: u64,
    installed_members: u64,
    pending_installations: u64,
    attention_installations: u64,
}

fn push_inventory_filters(builder: &mut QueryBuilder<'_, Any>, query: &ResourceUsageQuery) {
    if let Some(value) = query.user_id {
        builder.push(" AND c.user_id=");
        builder.push_bind(value.to_string());
    }
    if let Some(value) = query.primary_role {
        builder.push(" AND u.primary_role=");
        builder.push_bind(value.as_str());
    }
    if let Some(value) = query.installation_id {
        builder.push(" AND i.installation_id=");
        builder.push_bind(value.to_string());
    }
    if let Some(value) = query.resource_kind {
        builder.push(" AND r.kind=");
        builder.push_bind(value.as_str());
    }
    if let Some(value) = query.resource_id {
        builder.push(" AND i.resource_id=");
        builder.push_bind(value.to_string());
    }
    if let Some(value) = query.version_id {
        builder.push(" AND (i.applied_version_id=");
        builder.push_bind(value.to_string());
        builder.push(" OR i.desired_version_id=");
        builder.push_bind(value.to_string());
        builder.push(")");
    }
}

fn push_filtered_from(builder: &mut QueryBuilder<'_, Any>, query: &ResourceUsageQuery) {
    builder.push(" FROM telemetry_resource_attributions a JOIN telemetry_events e ON e.id=a.event_id JOIN resources r ON r.id=a.resource_id JOIN resource_versions rv ON rv.id=a.version_id JOIN users u ON u.id=e.user_id WHERE e.project_id=");
    builder.push_bind(query.project_id.to_string());
    builder.push(" AND e.received_at>=");
    builder.push_bind(query.from.to_rfc3339());
    builder.push(" AND e.received_at<=");
    builder.push_bind(query.to.to_rfc3339());
    if let Some(v) = query.user_id {
        builder.push(" AND e.user_id=");
        builder.push_bind(v.to_string());
    }
    if let Some(v) = query.primary_role {
        builder.push(" AND e.primary_role_snapshot=");
        builder.push_bind(v.as_str());
    }
    if let Some(v) = query.resource_kind {
        builder.push(" AND r.kind=");
        builder.push_bind(v.as_str());
    }
    if let Some(v) = query.resource_id {
        builder.push(" AND a.resource_id=");
        builder.push_bind(v.to_string());
    }
    if let Some(v) = query.version_id {
        builder.push(" AND a.version_id=");
        builder.push_bind(v.to_string());
    }
    if let Some(v) = query.installation_id {
        builder.push(" AND e.installation_id=");
        builder.push_bind(v.to_string());
    }
    if let Some(v) = query.relation {
        builder.push(" AND a.relation=");
        builder.push_bind(v.as_str());
    }
    push_request_event_filters(builder, query);
}

fn push_scoped_events(builder: &mut QueryBuilder<'_, Any>, query: &ResourceUsageQuery) {
    push_events_for_scope(builder, query, query.scope);
}

fn push_events_for_scope(
    builder: &mut QueryBuilder<'_, Any>,
    query: &ResourceUsageQuery,
    scope: ResourceUsageScope,
) {
    match scope {
        ResourceUsageScope::All => push_received_events(builder, query),
        ResourceUsageScope::Governed => push_filtered_events(builder, query),
    }
}

fn push_filtered_events(builder: &mut QueryBuilder<'_, Any>, query: &ResourceUsageQuery) {
    push_received_events(builder, query);
    builder.push(
        " AND EXISTS (SELECT 1 FROM telemetry_resource_attributions a \
         JOIN resources r ON r.id=a.resource_id \
         JOIN resource_versions rv ON rv.id=a.version_id WHERE a.event_id=e.id",
    );
    if let Some(v) = query.resource_kind {
        builder.push(" AND r.kind=");
        builder.push_bind(v.as_str());
    }
    if let Some(v) = query.resource_id {
        builder.push(" AND a.resource_id=");
        builder.push_bind(v.to_string());
    }
    if let Some(v) = query.version_id {
        builder.push(" AND a.version_id=");
        builder.push_bind(v.to_string());
    }
    if let Some(v) = query.relation {
        builder.push(" AND a.relation=");
        builder.push_bind(v.as_str());
    }
    builder.push(")");
}

fn push_received_events(builder: &mut QueryBuilder<'_, Any>, query: &ResourceUsageQuery) {
    builder.push(" FROM telemetry_events e JOIN users u ON u.id=e.user_id WHERE e.project_id=");
    builder.push_bind(query.project_id.to_string());
    builder.push(" AND e.received_at>=");
    builder.push_bind(query.from.to_rfc3339());
    builder.push(" AND e.received_at<=");
    builder.push_bind(query.to.to_rfc3339());
    if let Some(v) = query.user_id {
        builder.push(" AND e.user_id=");
        builder.push_bind(v.to_string());
    }
    if let Some(v) = query.primary_role {
        builder.push(" AND e.primary_role_snapshot=");
        builder.push_bind(v.as_str());
    }
    if let Some(v) = query.installation_id {
        builder.push(" AND e.installation_id=");
        builder.push_bind(v.to_string());
    }
    push_request_event_filters(builder, query);
}

fn push_request_event_filters(builder: &mut QueryBuilder<'_, Any>, query: &ResourceUsageQuery) {
    if let Some(status) = query.status {
        builder.push(
            " AND EXISTS (SELECT 1 FROM telemetry_events outcome_event \
             WHERE outcome_event.project_id=e.project_id \
             AND outcome_event.user_id=e.user_id \
             AND outcome_event.request_id=e.request_id \
             AND outcome_event.event_type='request' \
             AND outcome_event.status=",
        );
        builder.push_bind(status.as_str());
        builder.push(")");
    }
    if query.provider.is_some() || query.model.is_some() {
        builder.push(
            " AND EXISTS (SELECT 1 FROM telemetry_events model_event \
             WHERE model_event.project_id=e.project_id \
             AND model_event.user_id=e.user_id \
             AND model_event.request_id=e.request_id \
             AND model_event.event_type='model_call'",
        );
        if let Some(provider) = query.provider.as_deref() {
            builder.push(" AND model_event.provider=");
            builder.push_bind(provider.to_string());
        }
        if let Some(model) = query.model.as_deref() {
            builder.push(" AND model_event.model=");
            builder.push_bind(model.to_string());
        }
        builder.push(")");
    }
    if let Some(tool_name) = query.tool_name.as_deref() {
        builder.push(
            " AND EXISTS (SELECT 1 FROM telemetry_events tool_event \
             WHERE tool_event.project_id=e.project_id \
             AND tool_event.user_id=e.user_id \
             AND tool_event.request_id=e.request_id \
             AND tool_event.event_type='tool_call' \
             AND tool_event.tool_name=",
        );
        builder.push_bind(tool_name.to_string());
        builder.push(")");
    }
}

fn uuid(value: String) -> Option<Uuid> {
    Uuid::parse_str(&value).ok()
}
fn n(value: i64) -> u64 {
    value.max(0) as u64
}

fn request_identity_sql(kind: DatabaseKind, alias: &str) -> String {
    match kind {
        DatabaseKind::Mysql => format!("CONCAT({alias}.user_id, ':', {alias}.request_id)"),
        DatabaseKind::Postgres | DatabaseKind::Sqlite => {
            format!("{alias}.user_id || ':' || {alias}.request_id")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::request_identity_sql;
    use crate::core::dialect::DatabaseKind;

    #[test]
    fn request_identity_uses_native_concatenation() {
        assert_eq!(
            request_identity_sql(DatabaseKind::Mysql, "e"),
            "CONCAT(e.user_id, ':', e.request_id)"
        );
        for kind in [DatabaseKind::Postgres, DatabaseKind::Sqlite] {
            assert_eq!(
                request_identity_sql(kind, "event"),
                "event.user_id || ':' || event.request_id"
            );
        }
    }
}
