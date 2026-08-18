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

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta, Utc};
    use uuid::Uuid;

    use super::presence_sql;
    use crate::core::dialect::DatabaseKind;
    use crate::core::url::sqlite_shared_memory_url;
    use crate::Db;

    #[test]
    fn presence_query_uses_native_postgres_bind_markers() {
        let sql = presence_sql(DatabaseKind::Postgres);
        assert!(sql.contains("c.instance_id = $1"));
        assert!(sql.contains("c.last_seen_at >= $2"));
        assert!(sql.contains("c.last_seen_at <= $3"));
        assert!(!sql.contains('?'));
    }

    #[tokio::test]
    async fn presence_counts_recent_active_members_within_one_project() {
        let db = Db::connect(&sqlite_shared_memory_url(&format!(
            "dashboard_presence_{}",
            Uuid::new_v4().simple()
        )))
        .await
        .expect("connect dashboard test database");
        let project_id = seed_instance(&db, "presence-project").await;
        let other_project_id = seed_instance(&db, "other-project").await;
        let active_a = seed_user(&db, "active-a", "active").await;
        let active_b = seed_user(&db, "active-b", "active").await;
        let stale = seed_user(&db, "stale", "active").await;
        let disabled = seed_user(&db, "disabled", "disabled").await;
        let observed_at = DateTime::parse_from_rfc3339("2026-08-18T12:00:00Z")
            .expect("fixed timestamp")
            .with_timezone(&Utc);

        seed_installation(
            &db,
            project_id,
            active_a,
            observed_at - TimeDelta::seconds(180),
            "active-a-boundary",
        )
        .await;
        seed_installation(
            &db,
            project_id,
            active_a,
            observed_at - TimeDelta::seconds(10),
            "active-a-second-client",
        )
        .await;
        seed_installation(
            &db,
            project_id,
            active_b,
            observed_at - TimeDelta::seconds(179),
            "active-b",
        )
        .await;
        seed_installation(
            &db,
            project_id,
            stale,
            observed_at - TimeDelta::seconds(181),
            "stale",
        )
        .await;
        seed_installation(
            &db,
            project_id,
            disabled,
            observed_at - TimeDelta::seconds(1),
            "disabled",
        )
        .await;
        seed_installation(
            &db,
            other_project_id,
            active_b,
            observed_at - TimeDelta::seconds(1),
            "other-project",
        )
        .await;

        let presence = db
            .dashboard()
            .presence(Some(project_id), observed_at, 180)
            .await
            .expect("load heartbeat presence");
        assert_eq!(presence.clients_seen_recently, 3);
        assert_eq!(presence.members_seen_recently, 2);
        assert_eq!(presence.threshold_seconds, 180);
        assert_eq!(presence.observed_at, observed_at);
    }

    async fn seed_instance(db: &Db, label: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO instance (
                id, project_name, display_name, bind_host, bind_port,
                setup_completed, jwt_secret, created_at, updated_at
            ) VALUES (?, ?, ?, '127.0.0.1', 0, 0, 'test-secret',
                      '2026-08-18T00:00:00Z', '2026-08-18T00:00:00Z')
            "#,
        )
        .bind(id.to_string())
        .bind(label)
        .bind(label)
        .execute(db.pool())
        .await
        .expect("seed instance");
        id
    }

    async fn seed_user(db: &Db, label: &str, status: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, display_name, primary_role, status, created_at
            ) VALUES (?, ?, ?, 'user', ?, '2026-08-18T00:00:00Z')
            "#,
        )
        .bind(id.to_string())
        .bind(format!("{label}-{}@example.test", Uuid::new_v4().simple()))
        .bind(label)
        .bind(status)
        .execute(db.pool())
        .await
        .expect("seed user");
        id
    }

    async fn seed_installation(
        db: &Db,
        project_id: Uuid,
        user_id: Uuid,
        last_seen_at: DateTime<Utc>,
        label: &str,
    ) {
        let id = Uuid::new_v4();
        let timestamp = last_seen_at.to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO client_installations (
                id, instance_id, user_id, installation_key, display_name,
                platform, evoflux_version, connected_at, last_seen_at,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, 'test', 'test', ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(format!("{label}-{}", Uuid::new_v4().simple()))
        .bind(label)
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(db.pool())
        .await
        .expect("seed installation");
    }
}
