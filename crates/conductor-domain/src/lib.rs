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

pub mod client_installation;
pub mod core;
pub mod instance;
pub mod resource;
pub mod resource_semver;
pub mod role;
pub mod secret;
pub mod telemetry;
pub mod user;

pub use client_installation::{
    ClientHeartbeatRequest, ClientHeartbeatResponse, ClientInstallation, ClientInstallationSummary,
    ClientMember, ClientPlatform, ClientPolicy, ClientProject, ClientTelemetryPolicy,
    CollectionLevel, RegisterClientRequest, RegisterClientResponse, RegisteredInstallation,
};
pub use core::error::{ConductorError, Result};
pub use instance::{
    InstanceConfig, ProjectBranding, ProjectSettings, RealtimeSettings, SetupRequest,
    SetupSsoRequest, SetupStatus, SsoConfig, SsoProvider, UpdateInstanceRequest,
    UpdateNetworkRequest, UpdateSsoRequest,
};
pub use resource::{
    CreateDraftFileRequest, CreateResourceRequest, CreateResourceVersionRequest, DashboardSummary,
    DeleteDraftEntryRequest, DeprecateResourceVersionRequest, DiagnosticSeverity, DraftFile,
    DraftFileTree, EffectiveResourceVersion, ManagedResource, MoveDraftEntryRequest,
    ReleaseChannel, ReleaseResourceRequest, ReleaseResourceResult, ResourceAccessPolicy,
    ResourceChange, ResourceChangePage, ResourceCounts, ResourceDailyUsage, ResourceDiagnostic,
    ResourceFeedback, ResourceInstallationState, ResourceInventoryItem,
    ResourceInventoryMonitoring, ResourceInventoryMonitoringSummary,
    ResourceInventoryObservedState, ResourceInventoryRequest, ResourceInventoryResponse,
    ResourceKind, ResourceMemberUsage, ResourceMonitoring, ResourceMonitoringSummary,
    ResourceStatus, ResourceTargetMode, ResourceUsageBatchRequest, ResourceUsageBatchResponse,
    ResourceUsageEventRequest, ResourceUsageRejection, ResourceValidation, ResourceVersion,
    ResourceVersionLifecycleAction, ResourceVersionNotice, ResourceVersionStatus,
    ResourceVisibility, RestoreResourceVersionRequest, SaveDraftFileRequest, UpdateResourceRequest,
    UpsertResourceFeedbackRequest, VersionMode,
};
pub use resource_semver::{SemanticVersion, SemanticVersionError};
pub use role::{
    CreateSubRoleRequest, CreateTagRequest, PrimaryRole, SubRole, Tag, UpdateSubRoleRequest,
    UpdateTagRequest,
};
pub use secret::{ConnectionSecret, CreateSecretRequest, CreatedSecret, SecretScope};
pub use telemetry::{
    DailyTokenUsage, MemberActivityItem, MemberActivityResponse, MemberPresence,
    MemberRequestDetail, MemberToolUsage, MemberToolsSummary, MemberUsageSummary,
    ModelUsageBreakdown, ResourceUsageActivityItem, ResourceUsageAnalytics, ResourceUsageBreakdown,
    ResourceUsageDay, ResourceUsageMember, ResourceUsageModel, ResourceUsageRole,
    ResourceUsageTool, ResourceUsageTotals, TelemetryBatchRequest, TelemetryBatchResponse,
    TelemetryCostSource, TelemetryEventDetail, TelemetryEventRequest, TelemetryEventStatus,
    TelemetryEventType, TelemetryResourceAttributionDetail, TelemetryResourceRef,
    TelemetryResourceRelation, TelemetrySnapshot, TelemetryToolCategory, UNKNOWN_TELEMETRY_LABEL,
};
pub use user::{
    ApproveMemberRequest, AuthSession, ChangePasswordRequest, CreateMemberRequest, CreatedMember,
    MemberListQuery, MemberListResponse, ResetPasswordResponse, UpdateMemberRequest, User,
    UserStatus,
};
