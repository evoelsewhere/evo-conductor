use conductor_domain::{DashboardSummary, ResourceCounts};
use sqlx::{Any, Pool};

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
        let project_name = instance
            .map(|i| i.project_name)
            .unwrap_or_else(|| "Evo Conductor".into());

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

        let agents = self.published_resource_count("agent").await?;
        let skills = self.published_resource_count("skill").await?;
        let plugins = self.published_resource_count("plugin").await?;
        let workflows = self.published_resource_count("workflow").await?;

        let sso = InstanceRepo::new(self.pool.clone()).sso_config().await?;

        Ok(DashboardSummary {
            project_name,
            members_total: members_total as u32,
            members_online: members_online as u32,
            secrets_active: secrets_active as u32,
            resources: ResourceCounts {
                agents,
                skills,
                plugins,
                workflows,
            },
            sso_enabled: sso.enabled,
        })
    }

    async fn published_resource_count(&self, resource_kind: &str) -> Result<u32, sqlx::Error> {
        let sql = format!(
            "SELECT COUNT(*) FROM resources WHERE kind = {} AND status = 'published'",
            self.kind.bind_parameter(1),
        );
        let value: i64 = sqlx::query_scalar(&sql)
            .bind(resource_kind)
            .fetch_one(&self.pool)
            .await?;
        Ok(value.max(0).try_into().unwrap_or(u32::MAX))
    }
}
