mod access;
mod auth;
mod client;
mod dashboard;
mod health;
mod realtime;
mod resource_delivery;
mod resources;
mod secrets;
mod settings;
mod setup;
mod sso;
mod telemetry;
mod users;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, patch, post, put};
use axum::Router;

use crate::core::state::AppState;

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
        .route("/project", get(settings::get_project))
        .route(
            "/settings",
            get(settings::get_settings).patch(settings::update_settings),
        )
        .route("/settings/network", put(settings::update_network))
        .route("/dashboard", get(dashboard::summary))
        .route("/members", get(users::list).post(users::create))
        .route("/members/pending/count", get(users::pending_count))
        .route("/members/{id}", get(users::get).patch(users::update))
        .route(
            "/members/{id}/installations",
            get(client::list_member_installations),
        )
        .route(
            "/members/{id}/secrets",
            get(secrets::list_for_member).post(secrets::create_for_member),
        )
        .route(
            "/members/{id}/secrets/{secret_id}/revoke",
            post(secrets::revoke_for_member),
        )
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
        .route("/resources", get(resources::list).post(resources::create))
        .route(
            "/resources/plugins/inspect",
            post(resource_delivery::inspect_plugin_archive).layer(DefaultBodyLimit::max(
                crate::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            )),
        )
        .route(
            "/resources/plugins/import",
            post(resource_delivery::create_plugin_archive).layer(DefaultBodyLimit::max(
                crate::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            )),
        )
        .route(
            "/resources/imports/{kind}/inspect",
            post(resource_delivery::inspect_resource_archive).layer(DefaultBodyLimit::max(
                crate::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            )),
        )
        .route(
            "/resources/imports/{kind}",
            post(resource_delivery::create_resource_archive).layer(DefaultBodyLimit::max(
                crate::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            )),
        )
        .route("/resources/guides/{kind}", get(resource_delivery::guide))
        .route(
            "/resources/templates/{kind}",
            get(resource_delivery::template),
        )
        .route("/resources/{id}", patch(resources::update))
        .route("/resources/{id}/archive", post(resources::archive))
        .route(
            "/resources/{id}/draft/files",
            get(resource_delivery::draft_tree),
        )
        .route(
            "/resources/{id}/draft/files/{*path}",
            put(resource_delivery::save_draft_file),
        )
        .route(
            "/resources/{id}/draft/entries",
            post(resource_delivery::create_draft_file)
                .patch(resource_delivery::move_draft_entry)
                .delete(resource_delivery::delete_draft_entry),
        )
        .route(
            "/resources/{id}/draft/import",
            post(resource_delivery::import_archive).layer(DefaultBodyLimit::max(
                crate::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            )),
        )
        .route(
            "/resources/{id}/draft/validate",
            post(resource_delivery::validate),
        )
        .route("/resources/{id}/release", post(resource_delivery::release))
        .route(
            "/resources/{id}/versions",
            get(resources::versions).post(resources::create_version),
        )
        .route(
            "/resources/{id}/versions/{version_id}/publish",
            post(resources::publish_version),
        )
        .route(
            "/resources/{id}/versions/{version_id}/deprecate",
            post(resources::deprecate_version),
        )
        .route(
            "/resources/{id}/versions/{version_id}/restore-to-draft",
            post(resources::restore_version_to_draft),
        )
        .route(
            "/resources/{id}/access",
            get(resources::get_access).put(resources::set_access),
        )
        .route("/resources/{id}/monitoring", get(resources::monitoring))
        .route(
            "/resources/{id}/inventory",
            get(resources::inventory_monitoring),
        )
        .route(
            "/resources/{id}/feedback",
            get(resources::feedback).put(resources::upsert_feedback),
        )
        .route("/v1/subscribe/resources", get(resources::subscribe))
        .route("/v1/resources/changes", get(resource_delivery::changes))
        .route(
            "/v1/resources/{id}/versions/{version_id}",
            get(resource_delivery::version_payload),
        )
        .route(
            "/v1/resources/{id}/versions/{version_id}/artifact",
            get(resource_delivery::artifact),
        )
        .route("/v1/client/inventory", put(resource_delivery::inventory))
        .route("/v1/client/register", post(client::register))
        .route("/v1/client/heartbeat", post(client::heartbeat))
        .route("/v1/telemetry/batch", post(telemetry::ingest))
        .route("/v1/usage/resources", post(resources::ingest_usage))
        .route("/members/{id}/usage/summary", get(telemetry::usage_summary))
        .route("/members/{id}/activity", get(telemetry::activity))
        .route(
            "/members/{id}/activity/{request_id}",
            get(telemetry::request_detail),
        )
        .route("/members/{id}/tools", get(telemetry::tools_summary))
        .route("/analytics/resource-usage", get(telemetry::resource_usage))
        .route("/v1/realtime/events", get(realtime::events))
}
