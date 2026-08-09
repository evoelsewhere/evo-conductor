use conductor_domain::{DashboardSummary, ResourceCounts};
use sqlx::{Any, Pool};

use super::InstanceRepo;

#[derive(Clone)]
pub struct DashboardRepo {
    pool: Pool<Any>,
}

impl DashboardRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
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

        let agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE kind = 'agent'")
            .fetch_one(&self.pool)
            .await?;
        let skills: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE kind = 'skill'")
            .fetch_one(&self.pool)
            .await?;
        let mcp: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE kind = 'mcp'")
            .fetch_one(&self.pool)
            .await?;
        let workflows: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE kind = 'workflow'")
                .fetch_one(&self.pool)
                .await?;

        let sso = InstanceRepo::new(self.pool.clone()).sso_config().await?;

        Ok(DashboardSummary {
            project_name,
            members_total: members_total as u32,
            members_online: members_online as u32,
            secrets_active: secrets_active as u32,
            resources: ResourceCounts {
                agents: agents as u32,
                skills: skills as u32,
                mcp: mcp as u32,
                workflows: workflows as u32,
            },
            sso_enabled: sso.enabled,
        })
    }
}
