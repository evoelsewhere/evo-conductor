//! Building and inspecting database URLs.

use crate::core::constants::database::{
    SQLITE_SCHEME, SQLITE_SCHEME_LONG, SQLITE_SHARED_MEMORY_URL_TEMPLATE,
};

/// A named in-memory SQLite URL usable with a multi-connection pool.
///
/// See [`SQLITE_SHARED_MEMORY_URL_TEMPLATE`] for why the shape matters.
pub fn sqlite_shared_memory_url(name: &str) -> String {
    SQLITE_SHARED_MEMORY_URL_TEMPLATE.replace("{name}", name)
}

/// Strip the SQLite scheme and any query string, leaving the filesystem path.
///
/// Returns an empty string for URLs that name no file.
pub fn sqlite_path(database_url: &str) -> &str {
    let path = database_url
        .strip_prefix(SQLITE_SCHEME_LONG)
        .or_else(|| database_url.strip_prefix(SQLITE_SCHEME))
        .unwrap_or(database_url);
    let path = path.trim_start_matches('/');
    path.split('?').next().unwrap_or(path)
}
