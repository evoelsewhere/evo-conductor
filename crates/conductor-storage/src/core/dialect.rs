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

    /// Return the positional bind marker understood by this database.
    ///
    /// SQLx's runtime `Any` driver does not rewrite `?` markers for PostgreSQL,
    /// so queries assembled outside a compile-time database must select the
    /// native marker explicitly.
    pub fn bind_parameter(self, position: usize) -> String {
        assert!(position > 0, "bind positions are one-based");
        match self {
            Self::Postgres => format!("${position}"),
            Self::Sqlite | Self::Mysql => "?".to_string(),
        }
    }

    /// Return `count` comma-separated bind markers beginning at position 1.
    pub fn bind_parameters(self, count: usize) -> String {
        (1..=count)
            .map(|position| self.bind_parameter(position))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::DatabaseKind;

    #[test]
    fn bind_parameters_follow_native_dialect_syntax() {
        assert_eq!(DatabaseKind::Postgres.bind_parameters(3), "$1, $2, $3");
        assert_eq!(DatabaseKind::Sqlite.bind_parameters(3), "?, ?, ?");
        assert_eq!(DatabaseKind::Mysql.bind_parameters(3), "?, ?, ?");
    }
}
