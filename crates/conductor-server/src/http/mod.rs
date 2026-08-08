mod error;
mod extractors;
mod routes;
mod state;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

pub use state::AppState;

use crate::config::Config;

pub fn build_router(state: AppState, config: &Config) -> Router {
    let api = Router::new()
        .merge(routes::router())
        .with_state(state);

    Router::new()
        .nest("/api", api)
        .fallback_service(
            ServeDir::new(&config.web_dist)
                .not_found_service(ServeFile::new(config.web_dist.join("index.html"))),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

pub use error::{ApiError, ApiResult};
