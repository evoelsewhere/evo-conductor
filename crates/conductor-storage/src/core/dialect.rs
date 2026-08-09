//! Database dialect detection and connection helpers.

use crate::core::constants::database::{
    MYSQL_SCHEME, POSTGRES_SCHEME, POSTGRES_SCHEME_LONG, SQLITE_SCHEME, SQLITE_SCHEME_LONG,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
    Mysql,
}

impl DatabaseKind {
    pub fn detect(database_url: &str) -> Option<Self> {
        let url = database_url.split('?').next().unwrap_or(database_url);
        if url.starts_with(SQLITE_SCHEME) || url.starts_with(SQLITE_SCHEME_LONG) {
            Some(Self::Sqlite)
        } else if url.starts_with(POSTGRES_SCHEME) || url.starts_with(POSTGRES_SCHEME_LONG) {
            Some(Self::Postgres)
        } else if url.starts_with(MYSQL_SCHEME) {
            Some(Self::Mysql)
        } else {
            None
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
        }
    }
}
