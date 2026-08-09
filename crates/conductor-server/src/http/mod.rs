//! Transport layer: the router, its handlers, and its extractors.

mod extractors;
mod routes;

use axum::{
    http::{header, HeaderName, HeaderValue},
    Router,
};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::core::constants::http::{API_PREFIX, SPA_INDEX_FILE};
use crate::core::{AppState, Config};

pub fn build_router(state: AppState, config: &Config) -> Router {
    let api = Router::new().merge(routes::router()).with_state(state);

    Router::new()
        .nest(API_PREFIX, api)
        .fallback_service(
            ServeDir::new(&config.web_dist)
                .not_found_service(ServeFile::new(config.web_dist.join(SPA_INDEX_FILE))),
        )
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; connect-src 'self'; img-src 'self' https: data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:",
            ),
        ))
        .layer(TraceLayer::new_for_http())
}
