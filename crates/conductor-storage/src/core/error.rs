use std::fmt;

use uuid::Uuid;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedSecurityReason {
    InvalidUuid,
    UnknownValue,
    InvalidTimestamp,
    InvalidInteger,
    InvalidBoolean,
    MalformedPayload,
    EmptyCollection,
    DuplicateValue,
}

impl PersistedSecurityReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidUuid => "invalid_uuid",
            Self::UnknownValue => "unknown_value",
            Self::InvalidTimestamp => "invalid_timestamp",
            Self::InvalidInteger => "invalid_integer",
            Self::InvalidBoolean => "invalid_boolean",
            Self::MalformedPayload => "malformed_payload",
            Self::EmptyCollection => "empty_collection",
            Self::DuplicateValue => "duplicate_value",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedPrincipalField {
    Id,
    ProjectId,
    OwnerUserId,
    ResourceKind,
    ResourceStatus,
    PrimaryRole,
    Status,
    SessionVersion,
    MustChangePassword,
    LastSeenAt,
    CreatedAt,
}

impl PersistedPrincipalField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::ProjectId => "project_id",
            Self::OwnerUserId => "owner_user_id",
            Self::ResourceKind => "resource_kind",
            Self::ResourceStatus => "resource_status",
            Self::PrimaryRole => "primary_role",
            Self::Status => "status",
            Self::SessionVersion => "session_version",
            Self::MustChangePassword => "must_change_password",
            Self::LastSeenAt => "last_seen_at",
            Self::CreatedAt => "created_at",
        }
    }
}

/// A row was fetched successfully but cannot represent an authenticated principal.
///
/// The raw corrupt value is intentionally not retained. `row_id` is populated only
/// when the persisted UUID itself decoded successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPersistedPrincipal {
    pub row_id: Option<Uuid>,
    pub field: PersistedPrincipalField,
    pub reason: PersistedSecurityReason,
}

impl InvalidPersistedPrincipal {
    pub const fn new(
        row_id: Option<Uuid>,
        field: PersistedPrincipalField,
        reason: PersistedSecurityReason,
    ) -> Self {
        Self {
            row_id,
            field,
            reason,
        }
    }
}

impl fmt::Display for InvalidPersistedPrincipal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid persisted principal field {} ({})",
            self.field.as_str(),
            self.reason.as_str()
        )
    }
}

impl std::error::Error for InvalidPersistedPrincipal {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedCredentialField {
    Id,
    TokenHash,
    OwnerUserId,
    Scopes,
    LastUsedAt,
    ExpiresAt,
    RevokedAt,
    CreatedAt,
}

impl PersistedCredentialField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::TokenHash => "token_hash",
            Self::OwnerUserId => "owner_user_id",
            Self::Scopes => "scopes",
            Self::LastUsedAt => "last_used_at",
            Self::ExpiresAt => "expires_at",
            Self::RevokedAt => "revoked_at",
            Self::CreatedAt => "created_at",
        }
    }
}

/// A fetched connection credential contains corrupt authorization-critical state.
/// Raw hashes, scope JSON, timestamps, and malformed identifiers are never retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPersistedCredential {
    pub credential_id: Option<Uuid>,
    pub field: PersistedCredentialField,
    pub reason: PersistedSecurityReason,
}

impl InvalidPersistedCredential {
    pub const fn new(
        credential_id: Option<Uuid>,
        field: PersistedCredentialField,
        reason: PersistedSecurityReason,
    ) -> Self {
        Self {
            credential_id,
            field,
            reason,
        }
    }
}

impl fmt::Display for InvalidPersistedCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid persisted credential field {} ({})",
            self.field.as_str(),
            self.reason.as_str()
        )
    }
}

impl std::error::Error for InvalidPersistedCredential {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedResourceField {
    Id,
    ProjectId,
    OwnerUserId,
    Kind,
    Visibility,
    Status,
    Payload,
    DraftRevision,
    ReleaseChannel,
    VersionId,
    VersionStatus,
    ContentSize,
    PublishedAt,
    CreatedAt,
    UpdatedAt,
    AccessPolicy,
    ChangeSequence,
    ChangeAudience,
    InventoryInstallationId,
    InventoryDesiredVersionId,
    InventoryAppliedVersionId,
    InventoryObservedState,
    InventoryObservedAt,
    InventoryLastSeenAt,
    UsageOutcome,
    UsageDuration,
    UsageTokens,
    UsageOccurredAt,
}

impl PersistedResourceField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::ProjectId => "project_id",
            Self::OwnerUserId => "owner_user_id",
            Self::Kind => "kind",
            Self::Visibility => "visibility",
            Self::Status => "status",
            Self::Payload => "payload",
            Self::DraftRevision => "draft_revision",
            Self::ReleaseChannel => "release_channel",
            Self::VersionId => "version_id",
            Self::VersionStatus => "version_status",
            Self::ContentSize => "content_size",
            Self::PublishedAt => "published_at",
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::AccessPolicy => "access_policy",
            Self::ChangeSequence => "change_sequence",
            Self::ChangeAudience => "change_audience",
            Self::InventoryInstallationId => "inventory_installation_id",
            Self::InventoryDesiredVersionId => "inventory_desired_version_id",
            Self::InventoryAppliedVersionId => "inventory_applied_version_id",
            Self::InventoryObservedState => "inventory_observed_state",
            Self::InventoryObservedAt => "inventory_observed_at",
            Self::InventoryLastSeenAt => "inventory_last_seen_at",
            Self::UsageOutcome => "usage_outcome",
            Self::UsageDuration => "usage_duration",
            Self::UsageTokens => "usage_tokens",
            Self::UsageOccurredAt => "usage_occurred_at",
        }
    }
}

/// A fetched resource row contains corrupt state used by authorization or delivery.
///
/// Raw payloads and malformed identifiers are intentionally not retained. The
/// resource ID is populated only after that UUID has itself decoded safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPersistedResource {
    pub resource_id: Option<Uuid>,
    pub field: PersistedResourceField,
    pub reason: PersistedSecurityReason,
}

impl InvalidPersistedResource {
    pub const fn new(
        resource_id: Option<Uuid>,
        field: PersistedResourceField,
        reason: PersistedSecurityReason,
    ) -> Self {
        Self {
            resource_id,
            field,
            reason,
        }
    }
}

impl fmt::Display for InvalidPersistedResource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid persisted resource field {} ({})",
            self.field.as_str(),
            self.reason.as_str()
        )
    }
}

impl std::error::Error for InvalidPersistedResource {}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database operation failed")]
    Database(
        #[source]
        #[from]
        sqlx::Error,
    ),
    #[error(transparent)]
    InvalidPersistedPrincipal(#[from] InvalidPersistedPrincipal),
    #[error(transparent)]
    InvalidPersistedCredential(#[from] InvalidPersistedCredential),
    #[error(transparent)]
    InvalidPersistedResource(#[from] InvalidPersistedResource),
    #[error("storage serialization failed")]
    Serialization(#[source] serde_json::Error),
}

impl StorageError {
    pub const fn is_operational(&self) -> bool {
        matches!(self, Self::Database(_) | Self::Serialization(_))
    }
}
