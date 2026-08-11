//! Governed resource lifecycle validation and stable error codes.

pub const MAX_DEPRECATION_REASON_LENGTH: usize = 500;
pub const RESOURCE_MODE_SCOPE_FILENAME: &str = ".evoflux.json";

pub const ERROR_RESOURCE_ARCHIVED: &str = "resource_is_archived";
pub const ERROR_ACTIVE_RELEASE_DEPRECATION: &str = "active_release_cannot_be_deprecated";
pub const ERROR_VERSION_ALREADY_DEPRECATED: &str = "version_already_deprecated";
pub const ERROR_ONLY_RELEASED_LIFECYCLE: &str = "only_released_versions_support_lifecycle_actions";
pub const ERROR_DEPRECATED_CONFIRMATION_REQUIRED: &str = "deprecated_version_confirmation_required";
pub const ERROR_DRAFT_REVISION_CONFLICT: &str = "draft_revision_conflict";
pub const ERROR_VERSION_SOURCE_NOT_RESTORABLE: &str = "version_source_is_not_restorable";
