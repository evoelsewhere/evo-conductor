use sqlx::any::AnyPoolOptions;
use sqlx::{Any, Pool};

use crate::core::constants::database::{
    ENV_POOL_MAX_CONNECTIONS, POOL_MAX_CONNECTIONS, SQLITE_BUSY_TIMEOUT_PRAGMA,
    SQLITE_FOREIGN_KEYS_PRAGMA, SQLITE_MEMORY_PATH, SQLITE_SYNCHRONOUS_PRAGMA, SQLITE_WAL_PRAGMA,
};
use crate::core::dialect::DatabaseKind;
use crate::core::url::{normalize_database_url, sqlite_path};
use crate::migrate;
use crate::repos::{
    AiPolicyRepo, AnalyticsViewRepo, ClientInstallationRepo, DashboardRepo,
    InstallationContactRepo, InstanceRepo, JiraStatusHistoryRepo, JiraTaskRepo, MemberAccessRepo,
    ModelPriceRepo, ProjectIdCache, ResourceRepo, ResourceUsageRepo, RoleRepo, SecretRepo,
    SpendLimitRepo, TaskActivationRepo, TelemetryRepo, UserRepo,
};

/// Database handle. Cheap to clone (shares the connection pool).
///
/// Supports SQLite (default), Postgres, and MySQL via `CONDUCTOR_DATABASE_URL`.
#[derive(Clone)]
pub struct Db {
    pool: Pool<Any>,
    kind: DatabaseKind,
    /// Shared by every `instance()` repo, so the project id resolves once per
    /// process rather than once or twice per request.
    project_id_cache: ProjectIdCache,
}

impl Db {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        // Both SQLite scheme spellings reach here; the driver only opens one
        // of them for an absolute path. See `normalize_database_url`.
        let database_url = &normalize_database_url(database_url);
        let kind = DatabaseKind::detect(database_url).ok_or_else(|| {
            sqlx::Error::Configuration(
                format!(
                    "unsupported database URL (use sqlite:, postgres://, or mysql://): {database_url}"
                )
                .into(),
            )
        })?;

        if kind == DatabaseKind::Sqlite {
            ensure_sqlite_parent_dir(database_url);
        }

        // Required once process-wide for sqlx::Any (sqlite/postgres/mysql drivers).
        sqlx::any::install_default_drivers();

        let sqlite = kind == DatabaseKind::Sqlite;
        let pool = AnyPoolOptions::new()
            .max_connections(pool_max_connections())
            .after_connect(move |connection, _metadata| {
                Box::pin(async move {
                    if sqlite {
                        sqlx::query(SQLITE_FOREIGN_KEYS_PRAGMA)
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query(SQLITE_BUSY_TIMEOUT_PRAGMA)
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query(SQLITE_SYNCHRONOUS_PRAGMA)
                            .execute(&mut *connection)
                            .await?;
                    }
                    Ok(())
                })
            })
            .connect(database_url)
            .await?;

        if sqlite && is_file_backed_sqlite(database_url) {
            sqlx::query(SQLITE_WAL_PRAGMA).execute(&pool).await?;
        }

        migrate::run(&pool, kind).await?;
        tracing::info!(dialect = kind.as_str(), "database connected");
        Ok(Self {
            pool,
            kind,
            project_id_cache: ProjectIdCache::default(),
        })
    }

    pub fn pool(&self) -> &Pool<Any> {
        &self.pool
    }

    pub fn kind(&self) -> DatabaseKind {
        self.kind
    }

    pub fn instance(&self) -> InstanceRepo {
        InstanceRepo::with_project_id_cache(self.pool.clone(), self.project_id_cache.clone())
    }

    pub fn users(&self) -> UserRepo {
        UserRepo::new(self.pool.clone(), self.kind)
    }

    pub fn member_access(&self) -> MemberAccessRepo {
        MemberAccessRepo::new(self.pool.clone(), self.kind)
    }

    pub fn roles(&self) -> RoleRepo {
        RoleRepo::new(self.pool.clone())
    }

    pub fn secrets(&self) -> SecretRepo {
        SecretRepo::new(self.pool.clone())
    }

    pub fn resources(&self) -> ResourceRepo {
        ResourceRepo::new(self.pool.clone())
    }

    pub fn client_installations(&self) -> ClientInstallationRepo {
        ClientInstallationRepo::new(self.pool.clone())
    }

    pub fn dashboard(&self) -> DashboardRepo {
        DashboardRepo::new(self.pool.clone(), self.kind)
    }

    pub fn analytics_views(&self) -> AnalyticsViewRepo {
        AnalyticsViewRepo::new(self.pool.clone())
    }

    pub fn telemetry(&self) -> TelemetryRepo {
        TelemetryRepo::new(self.pool.clone(), self.kind)
    }

    pub fn spend_limits(&self) -> SpendLimitRepo {
        SpendLimitRepo::new(self.pool.clone(), self.kind)
    }

    pub fn ai_policies(&self) -> AiPolicyRepo {
        AiPolicyRepo::new(self.pool.clone(), self.kind)
    }

    pub fn jira_tasks(&self) -> JiraTaskRepo {
        JiraTaskRepo::new(self.pool.clone(), self.kind)
    }

    pub fn task_activations(&self) -> TaskActivationRepo {
        TaskActivationRepo::new(self.pool.clone())
    }

    pub fn jira_status_history(&self) -> JiraStatusHistoryRepo {
        JiraStatusHistoryRepo::new(self.pool.clone())
    }

    pub fn model_prices(&self) -> ModelPriceRepo {
        ModelPriceRepo::new(self.pool.clone(), self.kind)
    }

    pub fn installation_contacts(&self) -> InstallationContactRepo {
        InstallationContactRepo::new(self.pool.clone(), self.kind)
    }

    pub fn resource_usage(&self) -> ResourceUsageRepo {
        ResourceUsageRepo::new(self.pool.clone(), self.kind)
    }
}

/// Zero is rejected along with unset and non-numeric: sqlx refuses an empty pool.
fn pool_max_connections() -> u32 {
    std::env::var(ENV_POOL_MAX_CONNECTIONS)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(POOL_MAX_CONNECTIONS)
}

fn is_file_backed_sqlite(database_url: &str) -> bool {
    let path = sqlite_path(database_url);
    !path.is_empty() && path != SQLITE_MEMORY_PATH && !database_url.contains("mode=memory")
}

fn ensure_sqlite_parent_dir(database_url: &str) {
    let path = sqlite_path(database_url);
    if !path.is_empty() && path != SQLITE_MEMORY_PATH {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}
