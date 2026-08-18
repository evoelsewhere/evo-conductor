//! Evo Conductor HTTP server library.
//!
//! ```text
//! core/   configuration, state, errors, constants, route paths
//! http/   transport: router, handlers, extractors
//! ```

pub mod core;
pub mod http;
pub mod route_inventory;

pub use core::{ApiError, ApiResult, AppState, Config, RealtimeConfig};
pub use http::build_router;
