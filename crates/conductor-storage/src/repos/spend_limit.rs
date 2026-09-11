use chrono::{DateTime, Utc};
use conductor_domain::{evaluate_limit, LimitEvaluation, LimitPeriod, LimitScope, PrimaryRole};
use sqlx::Row;
use sqlx::{Any, Pool};
use uuid::Uuid;

use crate::core::dialect::DatabaseKind;
use crate::core::error::StorageResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpendLimit {
    pub id: Uuid,
    pub scope: LimitScope,
    /// Member id for a member limit, primary role for a role limit, empty for
    /// a project limit.
    pub subject_id: String,
    pub period: LimitPeriod,
    pub limit_usd_micros: u64,
    pub warn_percent: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpendLimitStatus {
    pub limit: SpendLimit,
    pub period_start: DateTime<Utc>,
    pub evaluation: LimitEvaluation,
}

#[derive(Debug, Clone)]
pub struct UpsertSpendLimit {
    pub scope: LimitScope,
    pub subject_id: String,
    pub period: LimitPeriod,
    pub limit_usd_micros: u64,
    pub warn_percent: u32,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct SpendLimitRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl SpendLimitRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    /// Create or replace the limit for a (scope, subject, period). One
    /// allowance per combination: two limits over the same window would
    /// disagree with each other and neither could be called authoritative.
    pub async fn upsert(
        &self,
        project_id: Uuid,
        request: &UpsertSpendLimit,
    ) -> StorageResult<SpendLimit> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let statement = match self.kind {
            DatabaseKind::Mysql => {
                r#"
                INSERT INTO spend_limits (
                    id, project_id, scope, subject_id, period,
                    limit_usd_micros, warn_percent, enabled, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                    limit_usd_micros = VALUES(limit_usd_micros),
                    warn_percent = VALUES(warn_percent),
                    enabled = VALUES(enabled),
                    updated_at = VALUES(updated_at)
                "#
            }
            DatabaseKind::Sqlite | DatabaseKind::Postgres => {
                r#"
                INSERT INTO spend_limits (
                    id, project_id, scope, subject_id, period,
                    limit_usd_micros, warn_percent, enabled, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT (project_id, scope, subject_id, period) DO UPDATE SET
                    limit_usd_micros = excluded.limit_usd_micros,
                    warn_percent = excluded.warn_percent,
                    enabled = excluded.enabled,
                    updated_at = excluded.updated_at
                "#
            }
        };
        sqlx::query(statement)
            .bind(id.to_string())
            .bind(project_id.to_string())
            .bind(request.scope.as_str())
            .bind(&request.subject_id)
            .bind(request.period.as_str())
            .bind(to_i64(request.limit_usd_micros))
            .bind(i64::from(request.warn_percent))
            .bind(i64::from(request.enabled))
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;

        Ok(self
            .find(
                project_id,
                request.scope,
                &request.subject_id,
                request.period,
            )
            .await?
            .expect("the row just written must be readable"))
    }

    pub async fn find(
        &self,
        project_id: Uuid,
        scope: LimitScope,
        subject_id: &str,
        period: LimitPeriod,
    ) -> StorageResult<Option<SpendLimit>> {
        let row = sqlx::query(
            r#"
            SELECT id, scope, subject_id, period, limit_usd_micros, warn_percent, enabled
            FROM spend_limits
            WHERE project_id = ? AND scope = ? AND subject_id = ? AND period = ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(scope.as_str())
        .bind(subject_id)
        .bind(period.as_str())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(map_limit))
    }

    pub async fn list(&self, project_id: Uuid) -> StorageResult<Vec<SpendLimit>> {
        let rows = sqlx::query(
            r#"
            SELECT id, scope, subject_id, period, limit_usd_micros, warn_percent, enabled
            FROM spend_limits
            WHERE project_id = ?
            ORDER BY scope ASC, subject_id ASC, period ASC
            "#,
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().filter_map(map_limit).collect())
    }

    pub async fn delete(
        &self,
        project_id: Uuid,
        scope: LimitScope,
        subject_id: &str,
        period: LimitPeriod,
    ) -> StorageResult<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM spend_limits
            WHERE project_id = ? AND scope = ? AND subject_id = ? AND period = ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(scope.as_str())
        .bind(subject_id)
        .bind(period.as_str())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Evaluate every enabled limit against spend in its own current period.
    ///
    /// Each limit is summed over its own window, so a daily and a monthly
    /// limit on the same member are both correct at once.
    pub async fn statuses(
        &self,
        project_id: Uuid,
        now: DateTime<Utc>,
    ) -> StorageResult<Vec<SpendLimitStatus>> {
        let mut statuses = Vec::new();
        for limit in self.list(project_id).await? {
            if !limit.enabled {
                continue;
            }
            let period_start = limit.period.start(now);
            let spent = self
                .spend_for(project_id, &limit, period_start, now)
                .await?;
            statuses.push(SpendLimitStatus {
                evaluation: evaluate_limit(spent, limit.limit_usd_micros, limit.warn_percent),
                period_start,
                limit,
            });
        }
        Ok(statuses)
    }

    /// Spend inside one window for one limit's subject.
    ///
    /// Uses the same authoritative cost expression as the analytics
    /// aggregates — Conductor's own price where it has one, the client's
    /// figure for rows priced before Conductor started pricing — so a limit
    /// can never disagree with the dashboard a member is shown.
    ///
    /// Filtered on `received_at`, matching the analytics panels: a member
    /// draining a week-old outbox would otherwise blow a daily limit for a
    /// day that has already closed.
    async fn spend_for(
        &self,
        project_id: Uuid,
        limit: &SpendLimit,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> StorageResult<u64> {
        let mut sql = String::from(
            "SELECT COALESCE(SUM(e.server_cost_usd_micros), 0) AS spent \
             FROM telemetry_events e \
             WHERE e.project_id = ? AND e.received_at >= ? AND e.received_at <= ?",
        );
        match limit.scope {
            LimitScope::Project => {}
            LimitScope::Member => sql.push_str(" AND e.user_id = ?"),
            LimitScope::Role => sql.push_str(
                " AND COALESCE(e.primary_role_snapshot, \
                 (SELECT u.primary_role FROM users u WHERE u.id = e.user_id)) = ?",
            ),
        }

        let mut query = sqlx::query(&sql)
            .bind(project_id.to_string())
            .bind(from.to_rfc3339())
            .bind(to.to_rfc3339());
        match limit.scope {
            LimitScope::Project => {}
            LimitScope::Member | LimitScope::Role => {
                query = query.bind(limit.subject_id.clone());
            }
        }
        let spent: i64 = query.fetch_one(&self.pool).await?.get("spent");
        Ok(spent.max(0) as u64)
    }
}

/// The role a role-scoped limit applies to, validated so a typo cannot create
/// a limit that silently matches nobody.
pub fn parse_role_subject(value: &str) -> Option<String> {
    PrimaryRole::parse(value).map(|role| role.as_str().to_string())
}

fn map_limit(row: sqlx::any::AnyRow) -> Option<SpendLimit> {
    let id: String = row.get("id");
    let scope: String = row.get("scope");
    let period: String = row.get("period");
    Some(SpendLimit {
        id: Uuid::parse_str(&id).ok()?,
        scope: LimitScope::parse(&scope)?,
        subject_id: row.get("subject_id"),
        period: LimitPeriod::parse(&period)?,
        limit_usd_micros: row.get::<i64, _>("limit_usd_micros").max(0) as u64,
        warn_percent: row.get::<i64, _>("warn_percent").clamp(0, 100) as u32,
        enabled: row.get::<i64, _>("enabled") != 0,
    })
}

fn to_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}
