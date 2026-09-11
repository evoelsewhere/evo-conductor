//! Evo Conductor HTTP server library.
//!
//! ```text
//! cli/    operator subcommands that run once and exit
//! core/   configuration, state, errors, constants, route paths
//! http/   transport: router, handlers, extractors
//! ```

pub mod cli;
pub mod core;
pub mod http;
pub mod route_inventory;

pub use core::{ApiError, ApiResult, AppState, Config, ModelPricingConfig, RealtimeConfig};
pub use http::build_router;
