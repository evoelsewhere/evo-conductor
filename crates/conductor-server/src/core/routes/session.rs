//! Requires a browser session: `Authorization: Bearer <jwt>`.

pub const AUTH_ME: &str = "/auth/me";
pub const AUTH_CHANGE_PASSWORD: &str = "/auth/change-password";
pub const SSO: &str = "/sso";
pub const PROJECT: &str = "/project";
pub const SETTINGS: &str = "/settings";
pub const DASHBOARD: &str = "/dashboard";
pub const MEMBERS: &str = "/members";
pub const MEMBERS_PENDING_COUNT: &str = "/members/pending/count";
pub const MEMBER: &str = "/members/{id}";
pub const MEMBER_APPROVE: &str = "/members/{id}/approve";
pub const MEMBER_DISABLE: &str = "/members/{id}/disable";
pub const MEMBER_ENABLE: &str = "/members/{id}/enable";
pub const MEMBER_RESET_PASSWORD: &str = "/members/{id}/reset-password";
pub const SUB_ROLES: &str = "/sub-roles";
pub const SUB_ROLE: &str = "/sub-roles/{id}";
pub const TAGS: &str = "/tags";
pub const TAG: &str = "/tags/{id}";
pub const TAG_ASSIGNMENTS: &str = "/tag-assignments/{entity_type}/{entity_id}";
pub const SECRETS: &str = "/secrets";
pub const SECRET_REVOKE: &str = "/secrets/{id}/revoke";
pub const RESOURCES: &str = "/resources";
pub const ANALYTICS_VIEWS: &str = "/analytics/views";
