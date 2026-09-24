use chrono::Utc;
use conductor_domain::AiPolicyScope;
use sqlx::Row;
use sqlx::{Any, Pool};
use uuid::Uuid;

use crate::core::dialect::DatabaseKind;
use crate::core::error::StorageResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPolicy {
    pub id: Uuid,
    pub scope: AiPolicyScope,
    /// Primary role for a role policy, empty for a project policy.
    pub subject_id: String,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub allowed_providers: Vec<String>,
    pub allowed_tools: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertAiPolicy {
    pub scope: AiPolicyScope,
    pub subject_id: String,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub allowed_providers: Vec<String>,
    pub allowed_tools: Vec<String>,
}

#[derive(Clone)]
pub struct AiPolicyRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl AiPolicyRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    /// Create or replace the policy for a (scope, subject). One policy per
    /// combination — two policies over the same subject would disagree with
    /// each other and neither could be called authoritative.
    pub async fn upsert(
        &self,
        project_id: Uuid,
        request: &UpsertAiPolicy,
    ) -> StorageResult<AiPolicy> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let allowed_providers = serde_json::to_string(&request.allowed_providers)
            .unwrap_or_else(|_| "[]".into());
        let allowed_tools =
            serde_json::to_string(&request.allowed_tools).unwrap_or_else(|_| "[]".into());
        let statement = match self.kind {
            DatabaseKind::Mysql => {
                r#"
                INSERT INTO project_ai_policies (
                    id, project_id, scope, subject_id, default_provider, default_model,
                    allowed_providers, allowed_tools, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                    default_provider = VALUES(default_provider),
                    default_model = VALUES(default_model),
                    allowed_providers = VALUES(allowed_providers),
                    allowed_tools = VALUES(allowed_tools),
                    updated_at = VALUES(updated_at)
                "#
            }
            DatabaseKind::Sqlite | DatabaseKind::Postgres => {
                r#"
                INSERT INTO project_ai_policies (
                    id, project_id, scope, subject_id, default_provider, default_model,
                    allowed_providers, allowed_tools, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT (project_id, scope, subject_id) DO UPDATE SET
                    default_provider = excluded.default_provider,
                    default_model = excluded.default_model,
                    allowed_providers = excluded.allowed_providers,
                    allowed_tools = excluded.allowed_tools,
                    updated_at = excluded.updated_at
                "#
            }
        };
        sqlx::query(statement)
            .bind(id.to_string())
            .bind(project_id.to_string())
            .bind(request.scope.as_str())
            .bind(&request.subject_id)
            .bind(&request.default_provider)
            .bind(&request.default_model)
            .bind(allowed_providers)
            .bind(allowed_tools)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;

        Ok(self
            .find(project_id, request.scope, &request.subject_id)
            .await?
            .expect("the row just written must be readable"))
    }

    pub async fn find(
        &self,
        project_id: Uuid,
        scope: AiPolicyScope,
        subject_id: &str,
    ) -> StorageResult<Option<AiPolicy>> {
        let row = sqlx::query(
            r#"
            SELECT id, scope, subject_id, default_provider, default_model,
                   allowed_providers, allowed_tools
            FROM project_ai_policies
            WHERE project_id = ? AND scope = ? AND subject_id = ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(scope.as_str())
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(map_policy))
    }

    pub async fn list(&self, project_id: Uuid) -> StorageResult<Vec<AiPolicy>> {
        let rows = sqlx::query(
            r#"
            SELECT id, scope, subject_id, default_provider, default_model,
                   allowed_providers, allowed_tools
            FROM project_ai_policies
            WHERE project_id = ?
            ORDER BY scope ASC, subject_id ASC
            "#,
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().filter_map(map_policy).collect())
    }

    pub async fn delete(
        &self,
        project_id: Uuid,
        scope: AiPolicyScope,
        subject_id: &str,
    ) -> StorageResult<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM project_ai_policies
            WHERE project_id = ? AND scope = ? AND subject_id = ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(scope.as_str())
        .bind(subject_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn map_policy(row: sqlx::any::AnyRow) -> Option<AiPolicy> {
    let id: String = row.get("id");
    let scope: String = row.get("scope");
    let allowed_providers: String = row.get("allowed_providers");
    let allowed_tools: String = row.get("allowed_tools");
    Some(AiPolicy {
        id: Uuid::parse_str(&id).ok()?,
        scope: AiPolicyScope::parse(&scope)?,
        subject_id: row.get("subject_id"),
        default_provider: row.get("default_provider"),
        default_model: row.get("default_model"),
        allowed_providers: serde_json::from_str(&allowed_providers).unwrap_or_default(),
        allowed_tools: serde_json::from_str(&allowed_tools).unwrap_or_default(),
    })
}
