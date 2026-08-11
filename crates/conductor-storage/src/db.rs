use sqlx::any::AnyPoolOptions;
use sqlx::{Any, Pool};

use crate::core::constants::database::{
    POOL_MAX_CONNECTIONS, SQLITE_FOREIGN_KEYS_PRAGMA, SQLITE_MEMORY_PATH,
};
use crate::core::dialect::DatabaseKind;
use crate::core::url::sqlite_path;
use crate::migrate;
use crate::repos::{
    ClientInstallationRepo, DashboardRepo, InstanceRepo, ResourceRepo, ResourceUsageRepo, RoleRepo,
    SecretRepo, TelemetryRepo, UserRepo,
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

        let pool = AnyPoolOptions::new()
            .max_connections(POOL_MAX_CONNECTIONS)
            .connect(database_url)
            .await?;

        if kind == DatabaseKind::Sqlite {
            sqlx::query(SQLITE_FOREIGN_KEYS_PRAGMA)
                .execute(&pool)
                .await?;
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
        UserRepo::new(self.pool.clone())
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

    pub fn telemetry(&self) -> TelemetryRepo {
        TelemetryRepo::new(self.pool.clone())
    }

    pub fn resource_usage(&self) -> ResourceUsageRepo {
        ResourceUsageRepo::new(self.pool.clone())
    }
}

fn ensure_sqlite_parent_dir(database_url: &str) {
    let path = sqlite_path(database_url);
    if !path.is_empty() && path != SQLITE_MEMORY_PATH {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}
