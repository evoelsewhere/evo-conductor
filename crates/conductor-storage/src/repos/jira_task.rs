use chrono::Utc;
use conductor_domain::JiraTask;
use sqlx::{Any, Pool, Row};
use uuid::Uuid;

use crate::core::dialect::DatabaseKind;
use crate::core::error::StorageResult;
use crate::core::mapping::parse_dt;

#[derive(Clone)]
pub struct JiraTaskRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl JiraTaskRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    /// Upserts the whole synced batch inside one transaction — the sync job
    /// calls this once per tick with everything `search_issues` paged back,
    /// so a task removed from Jira between two ticks just stops being
    /// touched (still visible in the report against historical events)
    /// rather than the mirror being wiped and rebuilt.
    pub async fn upsert_many(&self, project_id: Uuid, tasks: &[JiraTask]) -> StorageResult<()> {
        let mut tx = self.pool.begin().await?;
        let synced_at = Utc::now().to_rfc3339();
        for task in tasks {
            let statement = match self.kind {
                DatabaseKind::Mysql => {
                    r#"
                    INSERT INTO jira_tasks (
                        issue_key, project_id, title, issue_type, resolved_type, resolved_project,
                        assignee_display_name, assignee_email, assignee_account_id, parent_key,
                        status, jira_updated_at, synced_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                        title = VALUES(title),
                        issue_type = VALUES(issue_type),
                        resolved_type = VALUES(resolved_type),
                        resolved_project = VALUES(resolved_project),
                        assignee_display_name = VALUES(assignee_display_name),
                        assignee_email = VALUES(assignee_email),
                        assignee_account_id = VALUES(assignee_account_id),
                        parent_key = VALUES(parent_key),
                        status = VALUES(status),
                        jira_updated_at = VALUES(jira_updated_at),
                        synced_at = VALUES(synced_at)
                    "#
                }
                DatabaseKind::Sqlite | DatabaseKind::Postgres => {
                    r#"
                    INSERT INTO jira_tasks (
                        issue_key, project_id, title, issue_type, resolved_type, resolved_project,
                        assignee_display_name, assignee_email, assignee_account_id, parent_key,
                        status, jira_updated_at, synced_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT (issue_key) DO UPDATE SET
                        title = excluded.title,
                        issue_type = excluded.issue_type,
                        resolved_type = excluded.resolved_type,
                        resolved_project = excluded.resolved_project,
                        assignee_display_name = excluded.assignee_display_name,
                        assignee_email = excluded.assignee_email,
                        assignee_account_id = excluded.assignee_account_id,
                        parent_key = excluded.parent_key,
                        status = excluded.status,
                        jira_updated_at = excluded.jira_updated_at,
                        synced_at = excluded.synced_at
                    "#
                }
            };
            sqlx::query(statement)
                .bind(&task.issue_key)
                .bind(project_id.to_string())
                .bind(&task.title)
                .bind(&task.issue_type)
                .bind(&task.resolved_type)
                .bind(task.resolved_project.as_deref())
                .bind(task.assignee_display_name.as_deref())
                .bind(task.assignee_email.as_deref())
                .bind(task.assignee_account_id.as_deref())
                .bind(task.parent_key.as_deref())
                .bind(&task.status)
                .bind(task.jira_updated_at.map(|dt| dt.to_rfc3339()))
                .bind(&synced_at)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// All synced tasks for the project, newest Jira update first.
    pub async fn list(&self, project_id: Uuid) -> StorageResult<Vec<JiraTask>> {
        let rows = sqlx::query(
            "SELECT * FROM jira_tasks WHERE project_id = ?              ORDER BY jira_updated_at DESC, issue_key DESC",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(map_row).collect())
    }

    /// Case-insensitive substring match on `issue_key` or `title`, for the
    /// report page's own search box. Capped rather than paginated -- a
    /// result list this long is already not useful as a dropdown.
    pub async fn search(&self, project_id: Uuid, query: &str) -> StorageResult<Vec<JiraTask>> {
        let like = format!("%{}%", query.to_lowercase());
        let rows = sqlx::query(
            r#"SELECT * FROM jira_tasks
               WHERE project_id = ?
                 AND (LOWER(issue_key) LIKE ? OR LOWER(title) LIKE ?)
               ORDER BY jira_updated_at DESC, issue_key DESC
               LIMIT 25"#,
        )
        .bind(project_id.to_string())
        .bind(&like)
        .bind(&like)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(map_row).collect())
    }

    pub async fn find(&self, project_id: Uuid, issue_key: &str) -> StorageResult<Option<JiraTask>> {
        let row = sqlx::query("SELECT * FROM jira_tasks WHERE project_id = ? AND issue_key = ?")
            .bind(project_id.to_string())
            .bind(issue_key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| map_row(&row)))
    }
}

fn map_row(row: &sqlx::any::AnyRow) -> JiraTask {
    JiraTask {
        issue_key: row.get("issue_key"),
        title: row.get("title"),
        issue_type: row.get("issue_type"),
        resolved_type: row.get("resolved_type"),
        resolved_project: row.get("resolved_project"),
        assignee_display_name: row.get("assignee_display_name"),
        assignee_email: row.get("assignee_email"),
        assignee_account_id: row.get("assignee_account_id"),
        parent_key: row.get("parent_key"),
        status: row.get("status"),
        jira_updated_at: row
            .get::<Option<String>, _>("jira_updated_at")
            .map(parse_dt),
        synced_at: parse_dt(row.get("synced_at")),
    }
}
