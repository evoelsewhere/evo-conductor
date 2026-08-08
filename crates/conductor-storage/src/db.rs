use sqlx::any::AnyPoolOptions;
use sqlx::{Any, Pool};

use crate::dialect::DatabaseKind;
use crate::migrate;
use crate::repos::{
    DashboardRepo, InstanceRepo, ResourceRepo, RoleRepo, SecretRepo, UserRepo,
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
            .max_connections(10)
            .connect(database_url)
            .await?;

        if kind == DatabaseKind::Sqlite {
            sqlx::query("PRAGMA foreign_keys = ON;")
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

    pub fn dashboard(&self) -> DashboardRepo {
        DashboardRepo::new(self.pool.clone())
    }
}

fn ensure_sqlite_parent_dir(database_url: &str) {
    let path = database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))
        .unwrap_or(database_url);
    let path = path.trim_start_matches('/');
    let path = path.split('?').next().unwrap_or(path);
    if !path.is_empty() && path != ":memory:" {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}
