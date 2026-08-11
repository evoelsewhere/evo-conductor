//! What [`crate::migrate`] creates. REQ-001 replaces the mechanism; these lists
//! are what the replacement must still produce.

/// Every table the migration creates.
pub const TABLES: &[&str] = &[
    "instance",
    "sso_config",
    "users",
    "sub_roles",
    "user_sub_roles",
    "tags",
    "user_tags",
    "tag_assignments",
    "connection_secrets",
    "client_installations",
    "client_registration_idempotency",
    "resources",
    "resource_versions",
    "resource_access_rules",
    "resource_release_channels",
    "resource_beta_members",
    "resource_changes",
    "resource_version_events",
    "installation_resource_inventory",
    "resource_usage_events",
    "resource_feedback",
    "analytics_views",
    "member_inventory",
    "telemetry_events",
    "telemetry_resource_attributions",
];

/// Every index the migration declares. These run after the tables, so they fail
/// first when a pool hands out separate databases.
pub const INDEXES: &[&str] = &[
    "idx_users_status",
    "idx_users_primary_role",
    "idx_user_tags_tag",
    "idx_tag_assignments_entity",
    "idx_tag_assignments_tag",
    "idx_user_sub_roles_role",
    "idx_client_installations_user_seen",
    "idx_client_installations_instance_seen",
    "idx_client_registration_replay_window",
    "idx_telemetry_user_time",
    "idx_telemetry_request",
    "idx_telemetry_installation_time",
    "idx_telemetry_project_received",
    "idx_telemetry_resource_time",
    "idx_resource_changes_audience",
    "idx_resource_changes_resource",
    "idx_resource_version_events_resource",
    "idx_resource_inventory_state",
    "idx_analytics_views_project_visibility",
    "idx_analytics_views_owner",
];
