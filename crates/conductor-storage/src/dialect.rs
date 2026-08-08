//! Database dialect detection and connection helpers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
    Mysql,
}

impl DatabaseKind {
    pub fn detect(database_url: &str) -> Option<Self> {
        let url = database_url.split('?').next().unwrap_or(database_url);
        if url.starts_with("sqlite:") || url.starts_with("sqlite://") {
            Some(Self::Sqlite)
        } else if url.starts_with("postgres://")
            || url.starts_with("postgresql://")
        {
            Some(Self::Postgres)
        } else if url.starts_with("mysql://") {
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
