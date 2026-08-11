//! API paths, one file per authentication family.
//!
//! The router mounts these and tests assert against them, so a rename breaks
//! compilation rather than silently turning assertions into 404s. Paths are
//! relative to [`crate::core::constants::http::API_PREFIX`], matching how the
//! router nests them; use [`absolute`] for the path a caller sees.

pub mod client;
pub mod public;
pub mod session;

use crate::core::constants::http::API_PREFIX;

/// The path as a caller sees it, including the API prefix.
pub fn absolute(path: &str) -> String {
    format!("{API_PREFIX}{path}")
}

/// Every session-authenticated collection path that takes no parameter.
///
/// Authorization matrix tests iterate this, so an endpoint added to the router
/// without an entry here is a review failure.
pub const ALL_SESSION_ROUTES: &[&str] = &[
    session::AUTH_ME,
    session::SSO,
    session::PROJECT,
    session::SETTINGS,
    session::DASHBOARD,
    session::MEMBERS,
    session::MEMBERS_PENDING_COUNT,
    session::SUB_ROLES,
    session::TAGS,
    session::SECRETS,
    session::RESOURCES,
    session::ANALYTICS_VIEWS,
];
