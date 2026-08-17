//! Shared foundations for the server: configuration, application state, the
//! error type every handler returns, and the constants and route paths the
//! transport layer is built from.
//!
//! `http/` holds the transport itself — routers, handlers, extractors — and
//! depends on this module rather than the other way round.

pub mod artifacts;
pub mod authorization;
pub mod config;
pub mod constants;
pub mod error;
pub mod request_context;
pub mod resource_authoring;
pub mod routes;
pub mod state;

pub use config::{Config, RealtimeConfig};
pub use error::{ApiError, ApiResult};
pub use state::AppState;
