use chrono::{DateTime, TimeDelta, Utc};
use conductor_domain::{DashboardPresence, DashboardSummary, ResourceCounts};
use sqlx::{Any, Pool};
use uuid::Uuid;

use crate::core::dialect::DatabaseKind;

use super::InstanceRepo;

#[derive(Clone)]
pub struct DashboardRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl DashboardRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    pub async fn summary(&self) -> Result<DashboardSummary, sqlx::Error> {
        let instance = InstanceRepo::new(self.pool.clone()).get().await?;
        let (project_id, project_name) = match instance {
            Some(instance) => (Some(instance.id), instance.project_name),
            None => (None, "Evo Conductor".into()),
        };

        let members_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;

        let members_online: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM member_inventory WHERE evoflux_connected = 1")
                .fetch_one(&self.pool)
                .await
                .unwrap_or(0);

        let secrets_active: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM connection_secrets WHERE revoked_at IS NULL")
                .fetch_one(&self.pool)
                .await?;

        let resources = match project_id {
            Some(project_id) => ResourceCounts {
                agents: self.published_resource_count(project_id, "agent").await?,
                skills: self.published_resource_count(project_id, "skill").await?,
                plugins: self.published_resource_count(project_id, "plugin").await?,
                workflows: self
                    .published_resource_count(project_id, "workflow")
                    .await?,
            },
            None => ResourceCounts::default(),
        };

        let sso = InstanceRepo::new(self.pool.clone()).sso_config().await?;

        Ok(DashboardSummary {
            project_name,
            members_total: members_total as u32,
            members_online: members_online as u32,
            secrets_active: secrets_active as u32,
            resources,
            sso_enabled: sso.enabled,
        })
    }

    async fn published_resource_count(
        &self,
        project_id: Uuid,
        resource_kind: &str,
    ) -> Result<u32, sqlx::Error> {
        let sql = format!(
            "SELECT COUNT(*) FROM resources WHERE project_id = {} AND kind = {} AND status = 'published'",
            self.kind.bind_parameter(1),
            self.kind.bind_parameter(2),
        );
        let value: i64 = sqlx::query_scalar(&sql)
            .bind(project_id.to_string())
            .bind(resource_kind)
            .fetch_one(&self.pool)
            .await?;
        Ok(count(value))
    }

    pub async fn presence(
        &self,
        project_id: Option<Uuid>,
        observed_at: DateTime<Utc>,
        threshold_seconds: u32,
    ) -> Result<DashboardPresence, sqlx::Error> {
        let Some(project_id) = project_id else {
            return Ok(DashboardPresence {
                clients_seen_recently: 0,
                members_seen_recently: 0,
                threshold_seconds,
                observed_at,
            });
        };
        let cutoff = observed_at - TimeDelta::seconds(i64::from(threshold_seconds));
        let sql = presence_sql(self.kind);
        let (clients, members): (i64, i64) = sqlx::query_as(&sql)
            .bind(project_id.to_string())
            .bind(cutoff.to_rfc3339())
            .bind(observed_at.to_rfc3339())
            .fetch_one(&self.pool)
            .await?;
        Ok(DashboardPresence {
            clients_seen_recently: count(clients),
            members_seen_recently: count(members),
            threshold_seconds,
            observed_at,
        })
    }
}

fn count(value: i64) -> u32 {
    value.max(0).try_into().unwrap_or(u32::MAX)
}

fn presence_sql(kind: DatabaseKind) -> String {
    format!(
        r#"
        SELECT COUNT(DISTINCT c.id), COUNT(DISTINCT c.user_id)
        FROM client_installations c
        JOIN users u ON u.id = c.user_id
        WHERE c.instance_id = {}
          AND u.status = 'active'
          AND c.last_seen_at >= {}
          AND c.last_seen_at <= {}
        "#,
        kind.bind_parameter(1),
        kind.bind_parameter(2),
        kind.bind_parameter(3),
    )
}
