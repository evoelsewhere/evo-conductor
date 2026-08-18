use chrono::{DateTime, TimeDelta, Utc};
use conductor_domain::{
    DashboardFeedbackDistribution, DashboardFeedbackScope, DashboardFeedbackSummary,
    DashboardPresence, DashboardSummary, ResourceCounts,
};
use sqlx::{Any, Pool, Row};
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
        let sql = published_resource_count_sql(self.kind);
        let value: i64 = sqlx::query_scalar(&sql)
            .bind(project_id.to_string())
            .bind(resource_kind)
            .fetch_one(&self.pool)
            .await?;
        Ok(count(value))
    }

    async fn presence(
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

    async fn feedback(
        &self,
        project_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
    ) -> Result<DashboardFeedbackSummary, sqlx::Error> {
        let scope = if owner_user_id.is_some() {
            DashboardFeedbackScope::OwnedResources
        } else {
            DashboardFeedbackScope::Project
        };
        let Some(project_id) = project_id else {
            return Ok(empty_feedback(scope));
        };

        let sql = feedback_sql(self.kind, owner_user_id.is_some());
        let mut query = sqlx::query(&sql).bind(project_id.to_string());
        if let Some(owner_user_id) = owner_user_id {
            query = query.bind(owner_user_id.to_string());
        }
        let row = query.fetch_one(&self.pool).await?;
        let feedback_count = count(row.try_get("feedback_count")?);
        let positive_count = count(row.try_get("positive_count")?);
        let distribution = DashboardFeedbackDistribution {
            rating_1: count(row.try_get("rating_1")?),
            rating_2: count(row.try_get("rating_2")?),
            rating_3: count(row.try_get("rating_3")?),
            rating_4: count(row.try_get("rating_4")?),
            rating_5: count(row.try_get("rating_5")?),
        };
        Ok(DashboardFeedbackSummary {
            scope,
            count: feedback_count,
            // Deriving from portable integer buckets avoids PostgreSQL NUMERIC
            // and MySQL DECIMAL decoding differences in sqlx::Any.
            average_rating: feedback_average(&distribution, feedback_count),
            positive_count,
            positive_percent: (feedback_count > 0).then(|| {
                round_one_decimal(f64::from(positive_count) * 100.0 / f64::from(feedback_count))
            }),
            distribution,
        })
    }
}

fn empty_feedback(scope: DashboardFeedbackScope) -> DashboardFeedbackSummary {
    DashboardFeedbackSummary {
        scope,
        count: 0,
        average_rating: None,
        positive_count: 0,
        positive_percent: None,
        distribution: DashboardFeedbackDistribution::default(),
    }
}

fn count(value: i64) -> u32 {
    value.max(0).try_into().unwrap_or(u32::MAX)
}

fn round_one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn feedback_average(distribution: &DashboardFeedbackDistribution, count: u32) -> Option<f64> {
    if count == 0 {
        return None;
    }
    let weighted_sum = f64::from(distribution.rating_1)
        + f64::from(distribution.rating_2) * 2.0
        + f64::from(distribution.rating_3) * 3.0
        + f64::from(distribution.rating_4) * 4.0
        + f64::from(distribution.rating_5) * 5.0;
    Some(round_one_decimal(weighted_sum / f64::from(count)))
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

fn published_resource_count_sql(kind: DatabaseKind) -> String {
    format!(
        "SELECT COUNT(*) FROM resources WHERE project_id = {} AND kind = {} AND status = 'published'",
        kind.bind_parameter(1),
        kind.bind_parameter(2),
    )
}

fn feedback_sql(kind: DatabaseKind, owner_scoped: bool) -> String {
    let owner_clause = if owner_scoped {
        format!(" AND r.owner_user_id = {}", kind.bind_parameter(2))
    } else {
        String::new()
    };
    format!(
        r#"SELECT COUNT(*) AS feedback_count,
           COUNT(CASE WHEN f.rating >= 4 THEN 1 END) AS positive_count,
           COUNT(CASE WHEN f.rating = 1 THEN 1 END) AS rating_1,
           COUNT(CASE WHEN f.rating = 2 THEN 1 END) AS rating_2,
           COUNT(CASE WHEN f.rating = 3 THEN 1 END) AS rating_3,
           COUNT(CASE WHEN f.rating = 4 THEN 1 END) AS rating_4,
           COUNT(CASE WHEN f.rating = 5 THEN 1 END) AS rating_5
           FROM resource_feedback f
           JOIN resources r ON r.id = f.resource_id
           WHERE r.project_id = {}
             AND f.rating BETWEEN 1 AND 5{}"#,
        kind.bind_parameter(1),
        owner_clause,
    )
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta, Utc};
    use uuid::Uuid;

    use super::{feedback_sql, presence_sql, published_resource_count_sql};
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

    #[test]
    fn published_resource_query_uses_native_postgres_bind_markers() {
        let sql = published_resource_count_sql(DatabaseKind::Postgres);
        assert!(sql.contains("project_id = $1"));
        assert!(sql.contains("kind = $2"));
        assert!(!sql.contains('?'));
    }

    #[test]
    fn feedback_query_uses_native_bind_markers_without_decimal_average() {
        let project = feedback_sql(DatabaseKind::Postgres, false);
        assert!(project.contains("r.project_id = $1"));
        assert!(!project.contains("owner_user_id"));
        assert!(!project.contains('?'));

        let owned = feedback_sql(DatabaseKind::Postgres, true);
        assert!(owned.contains("r.project_id = $1"));
        assert!(owned.contains("r.owner_user_id = $2"));
        assert!(!owned.contains('?'));

        for kind in [
            DatabaseKind::Sqlite,
            DatabaseKind::Postgres,
            DatabaseKind::Mysql,
        ] {
            assert!(!feedback_sql(kind, true).contains("AVG("));
        }
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

        // The threshold is inclusive. Multiple clients for one user count as
        // multiple clients but only one recently seen member.
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

    #[tokio::test]
    async fn feedback_is_project_scoped_and_can_be_limited_to_owned_resources() {
        let db = Db::connect(&sqlite_shared_memory_url(&format!(
            "dashboard_feedback_{}",
            Uuid::new_v4().simple()
        )))
        .await
        .expect("connect dashboard test database");
        let project_id = seed_instance(&db, "feedback-project").await;
        let other_project_id = seed_instance(&db, "other-feedback-project").await;
        let owner = seed_user(&db, "owner", "active").await;
        let other_owner = seed_user(&db, "other-owner", "active").await;
        let reviewer_a = seed_user(&db, "reviewer-a", "active").await;
        let reviewer_b = seed_user(&db, "reviewer-b", "active").await;

        let owned_a = seed_resource(&db, project_id, owner, "owned-a").await;
        let owned_b = seed_resource(&db, project_id, owner, "owned-b").await;
        let other_owned = seed_resource(&db, project_id, other_owner, "other-owned").await;
        let cross_project = seed_resource(&db, other_project_id, owner, "cross-project").await;
        seed_feedback(&db, owned_a, reviewer_a, 5, "private-a").await;
        seed_feedback(&db, owned_b, reviewer_b, 2, "private-b").await;
        seed_feedback(&db, other_owned, reviewer_a, 4, "private-c").await;
        seed_feedback(&db, cross_project, reviewer_b, 1, "private-cross").await;

        let project = db
            .dashboard()
            .feedback(Some(project_id), None)
            .await
            .expect("project feedback");
        assert_eq!(
            project.scope,
            conductor_domain::DashboardFeedbackScope::Project
        );
        assert_eq!(project.count, 3);
        assert_eq!(project.average_rating, Some(3.7));
        assert_eq!(project.positive_count, 2);
        assert_eq!(project.positive_percent, Some(66.7));
        assert_eq!(project.distribution.rating_1, 0);

        let owned = db
            .dashboard()
            .feedback(Some(project_id), Some(owner))
            .await
            .expect("owned-resource feedback");
        assert_eq!(
            owned.scope,
            conductor_domain::DashboardFeedbackScope::OwnedResources
        );
        assert_eq!(owned.count, 2);
        assert_eq!(owned.average_rating, Some(3.5));
        assert_eq!(owned.positive_count, 1);
        assert_eq!(owned.positive_percent, Some(50.0));
        assert_eq!(owned.distribution.rating_2, 1);
        assert_eq!(owned.distribution.rating_5, 1);
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

    async fn seed_resource(db: &Db, project_id: Uuid, owner_user_id: Uuid, label: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO resources (
                id, project_id, kind, slug, name, version, owner_user_id,
                visibility, status, payload, created_at, updated_at
            ) VALUES (?, ?, 'skill', ?, ?, '1.0.0', ?, 'shared', 'published',
                      '{}', '2026-08-18T00:00:00Z', '2026-08-18T00:00:00Z')
            "#,
        )
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(format!("{label}-{}", Uuid::new_v4().simple()))
        .bind(label)
        .bind(owner_user_id.to_string())
        .execute(db.pool())
        .await
        .expect("seed resource");
        id
    }

    async fn seed_feedback(db: &Db, resource_id: Uuid, user_id: Uuid, rating: i64, comment: &str) {
        sqlx::query(
            r#"
            INSERT INTO resource_feedback (
                id, resource_id, resource_version, user_id, rating, comment,
                created_at, updated_at
            ) VALUES (?, ?, '1.0.0', ?, ?, ?,
                      '2026-08-18T00:00:00Z', '2026-08-18T00:00:00Z')
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(resource_id.to_string())
        .bind(user_id.to_string())
        .bind(rating)
        .bind(comment)
        .execute(db.pool())
        .await
        .expect("seed feedback");
    }
}
