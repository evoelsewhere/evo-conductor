use sqlx::any::AnyPoolOptions;
use sqlx::{Any, Pool};

use crate::core::constants::database::{
    POOL_MAX_CONNECTIONS, SQLITE_BUSY_TIMEOUT_PRAGMA, SQLITE_FOREIGN_KEYS_PRAGMA,
    SQLITE_MEMORY_PATH, SQLITE_SYNCHRONOUS_PRAGMA, SQLITE_WAL_PRAGMA,
};
use crate::core::dialect::DatabaseKind;
use crate::core::url::sqlite_path;
use crate::migrate;
use crate::repos::{
    AnalyticsViewRepo, ClientInstallationRepo, DashboardRepo, InstanceRepo, MemberAccessRepo,
    ResourceRepo, ResourceUsageRepo, RoleRepo, SecretRepo, TelemetryRepo, UserRepo,
};

/// Database handle. Cheap to clone (shares the connection pool).
///
/// Supports SQLite (default), Postgres, and MySQL via `CONDUCTOR_DATABASE_URL`.
#[derive(Clone)]
pub struct Db {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl Db {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
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
            .max_connections(POOL_MAX_CONNECTIONS)
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

        migrate::run(&pool).await?;
        tracing::info!(dialect = kind.as_str(), "database connected");
        Ok(Self { pool, kind })
    }

    pub fn pool(&self) -> &Pool<Any> {
        &self.pool
    }

    pub fn kind(&self) -> DatabaseKind {
        self.kind
    }

    pub fn instance(&self) -> InstanceRepo {
        InstanceRepo::new(self.pool.clone())
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
        DashboardRepo::new(self.pool.clone())
    }

    pub fn analytics_views(&self) -> AnalyticsViewRepo {
        AnalyticsViewRepo::new(self.pool.clone())
    }

    pub fn telemetry(&self) -> TelemetryRepo {
        TelemetryRepo::new(self.pool.clone(), self.kind)
    }

    pub fn resource_usage(&self) -> ResourceUsageRepo {
        ResourceUsageRepo::new(self.pool.clone())
    }
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
