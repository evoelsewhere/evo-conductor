use conductor_domain::JiraStatusChange;
use sqlx::{Any, Pool, Row};
use uuid::Uuid;

use crate::core::error::StorageResult;
use crate::core::mapping::parse_dt;

#[derive(Clone)]
pub struct JiraStatusHistoryRepo {
    pool: Pool<Any>,
}

impl JiraStatusHistoryRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    /// Wipes and reinserts one issue's whole history -- a changelog page is
    /// cheap and always authoritative, so there's no update-in-place case
    /// worth the extra bookkeeping.
    pub async fn replace_for_issue(
        &self,
        project_id: Uuid,
        issue_key: &str,
        changes: &[JiraStatusChange],
    ) -> StorageResult<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM jira_status_history WHERE project_id = ? AND issue_key = ?")
            .bind(project_id.to_string())
            .bind(issue_key)
            .execute(&mut *tx)
            .await?;
        for change in changes {
            sqlx::query(
                "INSERT INTO jira_status_history (id, project_id, issue_key, status, changed_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(project_id.to_string())
            .bind(issue_key)
            .bind(&change.status)
            .bind(change.changed_at.to_rfc3339())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Every synced issue's history for the project, oldest first per issue
    /// -- loaded once per report build and grouped by issue_key in memory,
    /// rather than one query per task row.
    pub async fn list_for_project(&self, project_id: Uuid) -> StorageResult<Vec<JiraStatusChange>> {
        let rows = sqlx::query(
            "SELECT issue_key, status, changed_at FROM jira_status_history
             WHERE project_id = ? ORDER BY issue_key ASC, changed_at ASC",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|row| JiraStatusChange {
                issue_key: row.get("issue_key"),
                status: row.get("status"),
                changed_at: parse_dt(row.get("changed_at")),
            })
            .collect())
    }
}
