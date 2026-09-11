//! Per-installation, per-day contact records, and the coverage they support.
//!
//! A spend report cannot be trusted without knowing whether every client that
//! should have reported actually did. Silence and zero are the same number,
//! and only one of them means the total is understated. `last_seen_at` cannot
//! settle it — a heartbeat overwrites it, so yesterday's liveness is gone by
//! this morning. These rows keep it.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use conductor_domain::{UsageCoverage, UsageCoverageState};
use sqlx::{Any, Pool, Row};
use uuid::Uuid;

use crate::core::dialect::DatabaseKind;
use crate::core::error::StorageResult;

/// The UTC calendar day a server-clock instant falls in, as `YYYY-MM-DD`.
///
/// Every stored timestamp is RFC 3339 in UTC, so the day is the first ten
/// characters — the same slice the daily analytics group by. Formatting it
/// here rather than slicing at each call site keeps the two definitions from
/// drifting apart.
pub fn contact_day_of(at: DateTime<Utc>) -> String {
    at.date_naive().to_string()
}

/// Record that an installation was in contact on the day `at` falls in.
///
/// Takes an executor rather than a pool so telemetry ingest can record the
/// contact inside the same transaction as the events. Coverage that could be
/// committed without its events — or the reverse — would be a record of a
/// state that never existed.
pub async fn record_contact<'e, E>(
    executor: E,
    kind: DatabaseKind,
    installation_id: Uuid,
    project_id: Uuid,
    at: DateTime<Utc>,
    telemetry_events: u32,
    heartbeats: u32,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Any>,
{
    let day = contact_day_of(at);
    let now = at.to_rfc3339();
    // `first_contact_at` is never moved forward and the counters accumulate,
    // so replaying a batch cannot rewrite when the day started.
    let sql = match kind {
        DatabaseKind::Mysql => {
            "INSERT INTO installation_contact_days (
                 installation_id, project_id, contact_day, first_contact_at,
                 last_contact_at, telemetry_events, heartbeats
             ) VALUES (?, ?, ?, ?, ?, ?, ?)
             ON DUPLICATE KEY UPDATE
                 last_contact_at = VALUES(last_contact_at),
                 telemetry_events = telemetry_events + VALUES(telemetry_events),
                 heartbeats = heartbeats + VALUES(heartbeats)"
        }
        DatabaseKind::Sqlite | DatabaseKind::Postgres => {
            "INSERT INTO installation_contact_days (
                 installation_id, project_id, contact_day, first_contact_at,
                 last_contact_at, telemetry_events, heartbeats
             ) VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (installation_id, contact_day) DO UPDATE SET
                 last_contact_at = excluded.last_contact_at,
                 telemetry_events = installation_contact_days.telemetry_events
                     + excluded.telemetry_events,
                 heartbeats = installation_contact_days.heartbeats + excluded.heartbeats"
        }
    };
    sqlx::query(sql)
        .bind(installation_id.to_string())
        .bind(project_id.to_string())
        .bind(&day)
        .bind(&now)
        .bind(&now)
        .bind(i64::from(telemetry_events))
        .bind(i64::from(heartbeats))
        .execute(executor)
        .await?;
    Ok(())
}

#[derive(Clone)]
pub struct InstallationContactRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl InstallationContactRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    pub async fn record(
        &self,
        installation_id: Uuid,
        project_id: Uuid,
        at: DateTime<Utc>,
        telemetry_events: u32,
        heartbeats: u32,
    ) -> StorageResult<()> {
        record_contact(
            &self.pool,
            self.kind,
            installation_id,
            project_id,
            at,
            telemetry_events,
            heartbeats,
        )
        .await?;
        Ok(())
    }

    /// How much of `from..=to` the fleet accounted for.
    ///
    /// Judged only over **closed** days. An installation that has not yet
    /// reported today is not missing data — the day is still happening — and
    /// counting it would leave every live project permanently "incomplete",
    /// which teaches operators to ignore the signal.
    ///
    /// An installation is expected from the day it registered, so onboarding
    /// a machine today does not retroactively put the last month in deficit.
    pub async fn coverage(
        &self,
        project_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<UsageCoverage, sqlx::Error> {
        let window_start = from.date_naive();
        // The last day that is entirely in the past, clipped to the window.
        let last_closed = now.date_naive() - Duration::days(1);
        let window_end = to.date_naive().min(last_closed);

        if window_end < window_start {
            return Ok(empty_coverage());
        }
        let first_day = window_start.to_string();
        let last_day = window_end.to_string();

        // Grouped by registration day, so this stays small even on a fleet of
        // 100,000 installations: the row count is bounded by how many
        // distinct days anyone ever registered on, not by the fleet size.
        let cohorts = sqlx::query(
            "SELECT SUBSTR(connected_at, 1, 10) AS start_day, COUNT(*) AS installations
             FROM client_installations
             WHERE instance_id = ? AND SUBSTR(connected_at, 1, 10) <= ?
             GROUP BY SUBSTR(connected_at, 1, 10)",
        )
        .bind(project_id.to_string())
        .bind(&last_day)
        .fetch_all(&self.pool)
        .await?;

        let mut expected_installation_days: u64 = 0;
        let mut expected_installations: u64 = 0;
        for row in cohorts {
            let start_day: String = row.get("start_day");
            let installations = row.get::<i64, _>("installations").max(0) as u64;
            let Ok(registered) = start_day.parse::<NaiveDate>() else {
                // A row whose timestamp will not parse cannot be placed on a
                // day, so it cannot be judged. Skipping keeps it out of both
                // sides of the ratio rather than counting it as a deficit.
                continue;
            };
            let first = registered.max(window_start);
            if first > window_end {
                continue;
            }
            let days = (window_end - first).num_days().max(0) as u64 + 1;
            expected_installation_days += days * installations;
            expected_installations += installations;
        }

        let covered_installation_days = count(
            sqlx::query(
                "SELECT COUNT(*) AS n FROM installation_contact_days
                 WHERE project_id = ? AND contact_day >= ? AND contact_day <= ?",
            )
            .bind(project_id.to_string())
            .bind(&first_day)
            .bind(&last_day)
            .fetch_one(&self.pool)
            .await?,
        );

        let silent_installations = count(
            sqlx::query(
                "SELECT COUNT(*) AS n FROM client_installations c
                 WHERE c.instance_id = ? AND SUBSTR(c.connected_at, 1, 10) <= ?
                   AND NOT EXISTS (
                     SELECT 1 FROM installation_contact_days d
                     WHERE d.installation_id = c.id
                       AND d.contact_day >= ? AND d.contact_day <= ?
                   )",
            )
            .bind(project_id.to_string())
            .bind(&last_day)
            .bind(&first_day)
            .bind(&last_day)
            .fetch_one(&self.pool)
            .await?,
        );

        // Contact rows for installations deleted since, or recorded outside
        // the expected set, must not push coverage past complete.
        let covered_installation_days = covered_installation_days.min(expected_installation_days);
        let missing_installation_days =
            expected_installation_days.saturating_sub(covered_installation_days);

        let covered_bps = if expected_installation_days == 0 {
            10_000
        } else {
            ((covered_installation_days as u128 * 10_000) / expected_installation_days as u128)
                as u32
        };
        let state = if expected_installation_days == 0 {
            UsageCoverageState::Unknown
        } else if missing_installation_days == 0 {
            UsageCoverageState::Complete
        } else {
            UsageCoverageState::Incomplete
        };

        Ok(UsageCoverage {
            state,
            through_day: Some(last_day),
            expected_installation_days,
            covered_installation_days,
            missing_installation_days,
            silent_installations,
            expected_installations,
            covered_bps,
        })
    }
}

fn count(row: sqlx::any::AnyRow) -> u64 {
    row.get::<i64, _>("n").max(0) as u64
}

fn empty_coverage() -> UsageCoverage {
    UsageCoverage {
        state: UsageCoverageState::Unknown,
        through_day: None,
        expected_installation_days: 0,
        covered_installation_days: 0,
        missing_installation_days: 0,
        silent_installations: 0,
        expected_installations: 0,
        covered_bps: 10_000,
    }
}
