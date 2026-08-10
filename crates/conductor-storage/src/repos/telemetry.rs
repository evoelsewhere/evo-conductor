use chrono::{DateTime, Utc};
use conductor_domain::{
    DailyTokenUsage, MemberActivityItem, MemberActivityResponse, MemberRequestDetail,
    MemberToolUsage, MemberToolsSummary, MemberUsageSummary, ModelUsageBreakdown,
    TelemetryBatchResponse, TelemetryEventDetail, TelemetryEventRequest, TelemetryEventStatus,
    TelemetryEventType, TelemetryToolCategory, UNKNOWN_TELEMETRY_LABEL,
};
use sqlx::{Any, Pool, Row};
use uuid::Uuid;

use crate::core::mapping::parse_dt;

#[derive(Clone)]
pub struct TelemetryRepo {
    pool: Pool<Any>,
}

impl TelemetryRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn ingest(
        &self,
        user_id: Uuid,
        installation_id: Uuid,
        events: &[TelemetryEventRequest],
    ) -> Result<TelemetryBatchResponse, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let mut accepted = 0u32;
        let mut duplicates = 0u32;
        let received_at = Utc::now().to_rfc3339();

        for event in events {
            let exists: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM telemetry_events WHERE id = ?")
                    .bind(event.event_id.to_string())
                    .fetch_one(&mut *tx)
                    .await?;
            if exists > 0 {
                duplicates += 1;
                continue;
            }

            sqlx::query(
                r#"
                INSERT INTO telemetry_events (
                    id, user_id, installation_id, request_id, session_id, event_type,
                    sequence, agent_name, provider, model, tokens_in, tokens_out,
                    cache_read_tokens, reasoning_tokens, tool_use_tokens, duration_ms,
                    tool_name, tool_category, status, error_category, reported_at,
                    received_at, tool_calls, active_agents
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
                "#,
            )
            .bind(event.event_id.to_string())
            .bind(user_id.to_string())
            .bind(installation_id.to_string())
            .bind(event.request_id.trim())
            .bind(event.session_id.as_deref())
            .bind(event.event_type.as_str())
            .bind(i64::from(event.sequence))
            .bind(event.agent_name.as_deref())
            .bind(event.provider.as_deref())
            .bind(event.model.as_deref())
            .bind(to_i64(event.tokens_in))
            .bind(to_i64(event.tokens_out))
            .bind(to_i64(event.cache_read_tokens))
            .bind(to_i64(event.reasoning_tokens))
            .bind(to_i64(event.tool_use_tokens))
            .bind(to_i64(event.duration_ms))
            .bind(event.tool_name.as_deref())
            .bind(event.tool_category.map(TelemetryToolCategory::as_str))
            .bind(event.status.as_str())
            .bind(event.error_category.as_deref())
            .bind(event.reported_at.to_rfc3339())
            .bind(&received_at)
            .bind(if event.event_type == TelemetryEventType::ToolCall {
                1i64
            } else {
                0i64
            })
            .execute(&mut *tx)
            .await?;
            accepted += 1;
        }

        tx.commit().await?;
        Ok(TelemetryBatchResponse {
            accepted,
            duplicates,
        })
    }

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
                   COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens
            FROM telemetry_events
            WHERE user_id = ? AND request_id IS NOT NULL
              AND reported_at >= ? AND reported_at <= ?
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
                   COALESCE(SUM(tokens_out), 0) AS tokens_out
            FROM telemetry_events
            WHERE user_id = ? AND event_type = ?
              AND reported_at >= ? AND reported_at <= ?
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
            }
        })
        .collect();

        let daily = sqlx::query(
            r#"
            SELECT SUBSTR(reported_at, 1, 10) AS date,
                   COUNT(DISTINCT request_id) AS requests,
                   COALESCE(SUM(tokens_in), 0) AS tokens_in,
                   COALESCE(SUM(tokens_out), 0) AS tokens_out
            FROM telemetry_events
            WHERE user_id = ? AND request_id IS NOT NULL
              AND reported_at >= ? AND reported_at <= ?
            GROUP BY SUBSTR(reported_at, 1, 10)
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
            models,
            daily,
        })
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
             WHERE user_id = ? AND request_id IS NOT NULL AND reported_at >= ? AND reported_at <= ?",
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
                   COALESCE(SUM(duration_ms), 0) AS duration_ms,
                   SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS errors,
                   SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS blocked
            FROM telemetry_events
            WHERE user_id = ? AND request_id IS NOT NULL
              AND reported_at >= ? AND reported_at <= ?
            GROUP BY request_id
            ORDER BY started_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(TelemetryEventType::ModelCall.as_str())
        .bind(TelemetryEventType::ToolCall.as_str())
        .bind(TelemetryEventStatus::Error.as_str())
        .bind(TelemetryEventStatus::Blocked.as_str())
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
                   provider, model, tokens_in, tokens_out, cache_read_tokens,
                   reasoning_tokens, tool_use_tokens, duration_ms, tool_name,
                   tool_category, status, error_category, reported_at
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

        let started_at = parse_dt(rows[0].get("reported_at"));
        let finished_at = parse_dt(rows[rows.len() - 1].get("reported_at"));
        let mut model_calls = 0;
        let mut tool_calls = 0;
        let mut tokens_in = 0u64;
        let mut tokens_out = 0u64;
        let mut duration_ms = 0u64;
        let mut request_status = TelemetryEventStatus::Success;
        let mut provider = None;
        let mut model = None;
        let session_id = rows[0].get("session_id");
        let mut events = Vec::with_capacity(rows.len());

        for row in rows {
            let event_type = TelemetryEventType::parse(row.get::<String, _>("event_type").as_str())
                .unwrap_or(TelemetryEventType::ModelCall);
            if event_type == TelemetryEventType::ModelCall {
                model_calls += 1;
                provider = provider.or_else(|| row.get("provider"));
                model = model.or_else(|| row.get("model"));
            } else {
                tool_calls += 1;
            }
            let event_tokens_in = non_negative(row.get::<i64, _>("tokens_in"));
            let event_tokens_out = non_negative(row.get::<i64, _>("tokens_out"));
            let event_duration = non_negative(row.get::<i64, _>("duration_ms"));
            let status = TelemetryEventStatus::parse(row.get::<String, _>("status").as_str())
                .unwrap_or(TelemetryEventStatus::Error);
            request_status = merge_request_status(request_status, status);
            tokens_in = tokens_in.saturating_add(event_tokens_in);
            tokens_out = tokens_out.saturating_add(event_tokens_out);
            duration_ms = duration_ms.saturating_add(event_duration);
            events.push(TelemetryEventDetail {
                event_id: Uuid::parse_str(row.get::<String, _>("id").as_str())
                    .unwrap_or_else(|_| Uuid::nil()),
                event_type,
                sequence: non_negative(row.get::<i64, _>("sequence")) as u32,
                agent_name: row.get("agent_name"),
                provider: row.get("provider"),
                model: row.get("model"),
                tokens_in: event_tokens_in,
                tokens_out: event_tokens_out,
                cache_read_tokens: non_negative(row.get::<i64, _>("cache_read_tokens")),
                reasoning_tokens: non_negative(row.get::<i64, _>("reasoning_tokens")),
                tool_use_tokens: non_negative(row.get::<i64, _>("tool_use_tokens")),
                duration_ms: event_duration,
                tool_name: row.get("tool_name"),
                tool_category: row
                    .get::<Option<String>, _>("tool_category")
                    .as_deref()
                    .and_then(TelemetryToolCategory::parse),
                status,
                error_category: row.get("error_category"),
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
                duration_ms,
                status: request_status,
            },
            events,
        }))
    }

    pub async fn tools_summary(
        &self,
        user_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<MemberToolsSummary, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT COALESCE(tool_name, ?) AS tool_name,
                   COALESCE(tool_category, ?) AS category,
                   COUNT(*) AS calls,
                   SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS successes,
                   SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS errors,
                   COALESCE(SUM(duration_ms), 0) AS total_duration_ms,
                   MAX(reported_at) AS last_used_at
            FROM telemetry_events
            WHERE user_id = ? AND event_type = ?
              AND reported_at >= ? AND reported_at <= ?
            GROUP BY tool_name, tool_category
            ORDER BY calls DESC, tool_name
            "#,
        )
        .bind(UNKNOWN_TELEMETRY_LABEL)
        .bind(TelemetryToolCategory::Other.as_str())
        .bind(TelemetryEventStatus::Success.as_str())
        .bind(TelemetryEventStatus::Error.as_str())
        .bind(user_id.to_string())
        .bind(TelemetryEventType::ToolCall.as_str())
        .bind(from.to_rfc3339())
        .bind(to.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        let tools: Vec<MemberToolUsage> = rows
            .into_iter()
            .map(|row| {
                let calls = non_negative(row.get::<i64, _>("calls"));
                let total_duration_ms = non_negative(row.get::<i64, _>("total_duration_ms"));
                MemberToolUsage {
                    tool_name: row.get("tool_name"),
                    category: TelemetryToolCategory::parse(
                        row.get::<String, _>("category").as_str(),
                    )
                    .unwrap_or(TelemetryToolCategory::Other),
                    calls,
                    successes: non_negative(row.get::<i64, _>("successes")),
                    errors: non_negative(row.get::<i64, _>("errors")),
                    average_duration_ms: total_duration_ms.checked_div(calls).unwrap_or_default(),
                    last_used_at: parse_dt(row.get("last_used_at")),
                }
            })
            .collect();
        let total_calls = tools.iter().map(|item| item.calls).sum();
        let successful_calls = tools.iter().map(|item| item.successes).sum();
        let failed_calls = tools.iter().map(|item| item.errors).sum();

        Ok(MemberToolsSummary {
            from,
            to,
            total_calls,
            successful_calls,
            failed_calls,
            tools,
        })
    }
}

fn map_activity(row: sqlx::any::AnyRow) -> MemberActivityItem {
    let tokens_in = non_negative(row.get::<i64, _>("tokens_in"));
    let tokens_out = non_negative(row.get::<i64, _>("tokens_out"));
    let errors = non_negative(row.get::<i64, _>("errors"));
    let blocked = non_negative(row.get::<i64, _>("blocked"));
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
        status: if errors > 0 {
            TelemetryEventStatus::Error
        } else if blocked > 0 {
            TelemetryEventStatus::Blocked
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
        _ => TelemetryEventStatus::Success,
    }
}

fn to_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

fn non_negative(value: i64) -> u64 {
    value.max(0) as u64
}
