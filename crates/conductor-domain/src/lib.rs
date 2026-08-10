//! Domain layer — entities, value objects, and domain errors.
//! No I/O, no framework dependencies.
//!
//! ```text
//! core/       cross-cutting foundations: constants, errors
//! instance    project identity, SSO configuration, setup
//! user        members and their lifecycle
//! role        primary roles, sub-roles, tags
//! secret      connection tokens and their scopes
//! resource    the distributable catalog
//! telemetry   usage and presence reporting
//! ```

pub mod core;
pub mod instance;
pub mod resource;
pub mod role;
pub mod secret;
pub mod telemetry;
pub mod user;

pub use core::error::{ConductorError, Result};
pub use instance::{
    InstanceConfig, ProjectBranding, ProjectSettings, SetupRequest, SetupSsoRequest, SetupStatus,
    SsoConfig, SsoProvider, UpdateInstanceRequest, UpdateSsoRequest,
};
pub use resource::{
    CreateResourceRequest, CreateResourceVersionRequest, DashboardSummary, ManagedResource,
    ResourceAccessPolicy, ResourceCounts, ResourceDailyUsage, ResourceFeedback, ResourceKind,
    ResourceMemberUsage, ResourceMonitoring, ResourceMonitoringSummary, ResourceStatus,
    ResourceUsageBatchRequest, ResourceUsageBatchResponse, ResourceUsageEventRequest,
    ResourceUsageRejection, ResourceVersion, ResourceVersionStatus, ResourceVisibility,
    UpdateResourceRequest, UpsertResourceFeedbackRequest,
};
pub use role::{
    CreateSubRoleRequest, CreateTagRequest, PrimaryRole, SubRole, Tag, UpdateSubRoleRequest,
    UpdateTagRequest,
};
pub use secret::{ConnectionSecret, CreateSecretRequest, CreatedSecret, SecretScope};
pub use telemetry::{MemberPresence, TelemetrySnapshot};
pub use user::{
    ApproveMemberRequest, AuthSession, ChangePasswordRequest, CreateMemberRequest, CreatedMember,
    MemberListQuery, MemberListResponse, ResetPasswordResponse, UpdateMemberRequest, User,
    UserStatus,
};
