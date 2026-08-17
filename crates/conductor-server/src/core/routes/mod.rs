//! Small API path helpers retained for focused tests.
//!
//! These constants are not an authorization inventory. The sealed classified
//! catalog in `http::authorization` is the only complete route-policy source.

pub mod client;
pub mod public;
pub mod session;

use crate::core::constants::http::API_PREFIX;

/// The path as a caller sees it, including the API prefix.
pub fn absolute(path: &str) -> String {
    format!("{API_PREFIX}{path}")
}
