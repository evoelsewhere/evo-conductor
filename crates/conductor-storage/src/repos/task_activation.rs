use chrono::{DateTime, Utc};
use sqlx::{Any, Pool, Row};
use uuid::Uuid;

use crate::core::error::StorageResult;
use crate::core::mapping::parse_dt;

/// "This member began this task, now" -- one row per call the
/// jira-task-assistant plugin makes to its own activation endpoint, using
/// its own connection token. This is the only place a task's usage window
/// boundary comes from; nothing in evoflux core records these.
#[derive(Debug, Clone)]
pub struct TaskActivation {
    pub issue_key: String,
    pub started_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct TaskActivationRepo {
    pool: Pool<Any>,
}

impl TaskActivationRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn record(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        issue_key: &str,
    ) -> StorageResult<()> {
        sqlx::query(
            "INSERT INTO task_activations (id, project_id, user_id, issue_key, started_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(issue_key.trim())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// One user's activations across the whole project, oldest first -- the
    /// report builder turns consecutive pairs into `[start_i, start_{i+1})`
    /// windows and attributes the last one through `to`. Not scoped to
    /// `[from, to]` itself: a window can start before the report period and
    /// still cover events inside it.
    pub async fn list_for_user(
        &self,
        project_id: Uuid,
        user_id: Uuid,
    ) -> StorageResult<Vec<TaskActivation>> {
        let rows = sqlx::query(
            "SELECT issue_key, started_at FROM task_activations
             WHERE project_id = ? AND user_id = ?
             ORDER BY started_at ASC",
        )
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|row| TaskActivation {
                issue_key: row.get("issue_key"),
                started_at: parse_dt(row.get("started_at")),
            })
            .collect())
    }

    /// Distinct issue keys that have ever been activated in this project --
    /// the bounded set of issues worth paging Jira's changelog for.
    pub async fn distinct_issue_keys(&self, project_id: Uuid) -> StorageResult<Vec<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT issue_key FROM task_activations WHERE project_id = ?",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|row| row.get("issue_key")).collect())
    }
}
