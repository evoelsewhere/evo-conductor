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
    "member_inventory",
    "telemetry_events",
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
];
