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
/// Both `sqlite:` and `sqlite://` are accepted, and neither carries an
/// authority, so whatever follows the scheme is already the path — a leading
/// slash in `sqlite:///var/lib/conductor.db` is part of it. Returns an empty
/// string for URLs that name no file.
pub fn sqlite_path(database_url: &str) -> &str {
    let path = database_url
        .strip_prefix(SQLITE_SCHEME_LONG)
        .or_else(|| database_url.strip_prefix(SQLITE_SCHEME))
        .unwrap_or(database_url);
    path.split('?').next().unwrap_or(path)
}

/// Rewrite a SQLite URL into the scheme form the driver accepts.
///
/// `sqlite://` reads its remainder as a URL authority, which an absolute
/// Windows path (`sqlite://C:\data\conductor.db`) is not: the driver then
/// fails with a bare "unable to open database file" while Conductor has
/// already created the directory for it. Both spellings mean the same file, so
/// normalize to the single-colon form rather than making the operator know
/// which one their platform tolerates. Non-SQLite URLs are returned untouched.
pub fn normalize_database_url(database_url: &str) -> String {
    let Some(rest) = database_url
        .strip_prefix(SQLITE_SCHEME_LONG)
        .or_else(|| database_url.strip_prefix(SQLITE_SCHEME))
    else {
        return database_url.to_string();
    };
    format!("{SQLITE_SCHEME}{rest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_survives_every_scheme_spelling() {
        assert_eq!(sqlite_path("sqlite:data/conductor.db"), "data/conductor.db");
        assert_eq!(
            sqlite_path("sqlite://data/conductor.db"),
            "data/conductor.db"
        );
        assert_eq!(
            sqlite_path("sqlite:data/conductor.db?mode=rwc"),
            "data/conductor.db"
        );
    }

    /// The leading slash belongs to the path, not to the scheme. Trimming it
    /// turned `/var/lib/conductor.db` into a relative path, so Conductor
    /// created its parent under the working directory while the driver opened
    /// the absolute one.
    #[test]
    fn an_absolute_path_keeps_its_leading_slash() {
        assert_eq!(
            sqlite_path("sqlite:///var/lib/conductor/conductor.db?mode=rwc"),
            "/var/lib/conductor/conductor.db"
        );
        assert_eq!(
            sqlite_path("sqlite:/var/lib/conductor.db"),
            "/var/lib/conductor.db"
        );
    }

    #[test]
    fn a_windows_path_keeps_its_drive_and_separators() {
        assert_eq!(
            sqlite_path(r"sqlite://C:\ProgramData\conductor\conductor.db?mode=rwc"),
            r"C:\ProgramData\conductor\conductor.db"
        );
    }

    #[test]
    fn urls_that_name_no_file_are_left_recognisable() {
        assert_eq!(sqlite_path("sqlite::memory:"), ":memory:");
        assert_eq!(
            sqlite_path(&sqlite_shared_memory_url("suite")),
            "file:suite"
        );
    }

    #[test]
    fn normalizing_collapses_the_authority_form_and_changes_nothing_else() {
        assert_eq!(
            normalize_database_url(r"sqlite://C:\data\conductor.db?mode=rwc"),
            r"sqlite:C:\data\conductor.db?mode=rwc"
        );
        for unchanged in [
            "sqlite:data/conductor.db?mode=rwc",
            "sqlite::memory:",
            "sqlite:file:suite?mode=memory&cache=shared",
            "postgres://user:pw@localhost/conductor",
            "mysql://user@localhost/conductor",
        ] {
            assert_eq!(normalize_database_url(unchanged), unchanged);
        }
    }

    /// An absolute POSIX path must still be absolute after normalizing, or the
    /// driver would open a file beside the working directory instead.
    #[test]
    fn normalizing_keeps_an_absolute_posix_path_absolute() {
        assert_eq!(
            normalize_database_url("sqlite:///var/lib/conductor.db"),
            "sqlite:/var/lib/conductor.db"
        );
        assert_eq!(
            sqlite_path(&normalize_database_url("sqlite:///var/lib/conductor.db")),
            "/var/lib/conductor.db"
        );
    }
}
