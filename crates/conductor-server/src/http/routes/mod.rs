mod auth;
mod dashboard;
mod health;
mod resources;
mod secrets;
mod setup;
mod sso;
mod sub_roles;
mod users;

use axum::routing::{get, post};
use axum::Router;

use crate::http::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/setup/status", get(setup::status))
        .route("/setup", post(setup::complete))
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        .route("/auth/sso/start", get(auth::sso_start))
        .route("/auth/sso/callback", get(auth::sso_callback))
        .route("/sso", get(sso::get_config))
        .route("/dashboard", get(dashboard::summary))
        .route("/members", get(users::list))
        .route("/sub-roles", get(sub_roles::list).post(sub_roles::create))
        .route("/secrets", get(secrets::list).post(secrets::create))
        .route("/secrets/{id}/revoke", post(secrets::revoke))
        .route("/resources", get(resources::list))
        .route("/v1/subscribe/resources", get(resources::subscribe))
}
