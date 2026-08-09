//! Authentication values shared by the crates that issue and verify sessions.

/// The HTTP authentication scheme, including its trailing space. Both the
/// session extractor and the connection-token path strip it, in different files.
pub const AUTH_SCHEME_BEARER: &str = "Bearer ";

/// Session lifetime issued at login and on secret rotation.
///
/// Shortened from 72 to 24 hours alongside the browser refresh flow; see
/// `fix(web): refresh and protect browser auth sessions`.
pub const DEFAULT_JWT_TTL_HOURS: i64 = 24;
