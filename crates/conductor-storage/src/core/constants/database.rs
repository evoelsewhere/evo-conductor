//! Connection strings and pool sizing.

/// URL scheme prefixes recognised by [`crate::DatabaseKind::detect`]. Written in
/// two places before this existed: dialect detection and SQLite path handling.
pub const SQLITE_SCHEME: &str = "sqlite:";
pub const SQLITE_SCHEME_LONG: &str = "sqlite://";
pub const POSTGRES_SCHEME: &str = "postgres://";
pub const POSTGRES_SCHEME_LONG: &str = "postgresql://";
pub const MYSQL_SCHEME: &str = "mysql://";

/// The SQLite path meaning "no file at all".
pub const SQLITE_MEMORY_PATH: &str = ":memory:";

/// Connections the pool may open.
pub const POOL_MAX_CONNECTIONS: u32 = 10;

/// SQLite does not enforce foreign keys unless asked, per connection.
pub const SQLITE_FOREIGN_KEYS_PRAGMA: &str = "PRAGMA foreign_keys = ON;";

/// Wait for the current SQLite writer instead of surfacing a transient
/// `database is locked` error to concurrent telemetry and inventory clients.
pub const SQLITE_BUSY_TIMEOUT_PRAGMA: &str = "PRAGMA busy_timeout = 30000;";

/// WAL lets readers continue while the single SQLite writer commits. It is a
/// database-level setting and is enabled once for file-backed databases.
pub const SQLITE_WAL_PRAGMA: &str = "PRAGMA journal_mode = WAL;";

/// Keep WAL durability appropriate for an application database without an
/// fsync for every page; transaction commits remain durable through the WAL.
pub const SQLITE_SYNCHRONOUS_PRAGMA: &str = "PRAGMA synchronous = NORMAL;";

/// The only in-memory URL shape usable with a pool larger than one connection.
///
/// A plain `sqlite::memory:` URL gives every pooled connection its own private
/// database, so `CREATE TABLE` and the `CREATE INDEX` that follows land on
/// different connections and migration fails with `no such table: main.users`.
/// A named database with a shared cache is visible from all of them.
///
/// `{name}` is replaced by the caller. This is a property of the storage layer,
/// not of any one test, which is why it lives here.
pub const SQLITE_SHARED_MEMORY_URL_TEMPLATE: &str = "sqlite:file:{name}?mode=memory&cache=shared";
