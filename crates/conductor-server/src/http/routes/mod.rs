mod access;
mod auth;
mod dashboard;
mod health;
mod resources;
mod secrets;
mod settings;
mod setup;
mod sso;
mod users;

use axum::routing::{get, patch, post};
use axum::Router;

use crate::http::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/setup/status", get(setup::status))
        .route("/setup", post(setup::complete))
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        .route("/auth/change-password", post(auth::change_password))
        .route("/auth/sso/start", get(auth::sso_start))
        .route("/auth/sso/callback", get(auth::sso_callback))
        .route("/sso", get(sso::get_config).put(settings::update_sso))
        .route(
            "/settings",
            get(settings::get_settings).patch(settings::update_settings),
        )
        .route("/dashboard", get(dashboard::summary))
        .route("/members", get(users::list).post(users::create))
        .route("/members/pending/count", get(users::pending_count))
        .route("/members/{id}", get(users::get).patch(users::update))
        .route("/members/{id}/approve", post(users::approve))
        .route("/members/{id}/disable", post(users::disable))
        .route("/members/{id}/enable", post(users::enable))
        .route("/members/{id}/reset-password", post(users::reset_password))
        .route(
            "/sub-roles",
            get(access::list_sub_roles).post(access::create_sub_role),
        )
        .route(
            "/sub-roles/{id}",
            patch(access::update_sub_role).delete(access::delete_sub_role),
        )
        .route("/tags", get(access::list_tags).post(access::create_tag))
        .route(
            "/tags/{id}",
            patch(access::update_tag).delete(access::delete_tag),
        )
        .route(
            "/tag-assignments/{entity_type}/{entity_id}",
            get(access::get_entity_tags).put(access::set_entity_tags),
        )
        .route("/secrets", get(secrets::list).post(secrets::create))
        .route("/secrets/{id}/revoke", post(secrets::revoke))
        .route("/resources", get(resources::list))
        .route("/v1/subscribe/resources", get(resources::subscribe))
}
