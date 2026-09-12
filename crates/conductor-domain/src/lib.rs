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

pub mod analytics;
pub mod authorization;
pub mod client_installation;
pub mod core;
pub mod dashboard;
pub mod instance;
pub mod pricing;
pub mod resource;
pub mod resource_semver;
pub mod role;
pub mod secret;
pub mod spend;
pub mod telemetry;
pub mod user;

pub use analytics::{
    validate_analytics_view_metadata, AnalyticsComparison, AnalyticsDashboardDensity,
    AnalyticsDashboardPreset, AnalyticsDateRange, AnalyticsDimension, AnalyticsMetric,
    AnalyticsQuery, AnalyticsView, AnalyticsViewDefinition, AnalyticsViewVisibility,
    AnalyticsVisualization, AnalyticsWidget, AnalyticsWidgetSize, CreateAnalyticsViewRequest,
    UpdateAnalyticsViewRequest, ANALYTICS_VIEW_SCHEMA_VERSION,
};
pub use authorization::{
    evaluate_policy, grants_for_role, role_has_permission, scope_is_role_compatible,
    AuthenticationKind, AuthorizationAction, AuthorizationTarget, AuthorizationTargetSummary,
    ConnectionPolicyRequirement, ConstraintExpr, DecisionReason, DeclaredRequirement,
    LifecycleState, NonEmptySet, PermissionAlternative, PermissionGrant, PermissionKey,
    PolicyDecision, PolicyDefinitionError, PolicyInput, PolicyRequirement, ResponseProjection,
    TargetConstraint, TargetType, UnknownPolicyValue, V1_POLICY_REVISION,
};
pub use client_installation::{
    ClientHeartbeatRequest, ClientHeartbeatResponse, ClientInstallation, ClientInstallationSummary,
    ClientMember, ClientPlatform, ClientPolicy, ClientProject, ClientTelemetryPolicy,
    CollectionLevel, RegisterClientRequest, RegisterClientResponse, RegisteredInstallation,
};
pub use core::error::{ConductorError, Result};
pub use dashboard::{
    DashboardFeedbackDistribution, DashboardFeedbackScope, DashboardFeedbackSummary,
    DashboardHostMetrics, DashboardHostMetricsScope, DashboardPresence, DashboardRealtime,
    DashboardRealtimeScope, DashboardSummary, DASHBOARD_PRESENCE_THRESHOLD_SECONDS,
};
pub use instance::{
    AzureBlobStorageSettings, DataPolicySettings, GitAuthMode, GitStorageSettings, InstanceConfig,
    LocalStorageSettings, ProjectBranding, ProjectSettings, RealtimeSettings, S3StorageSettings,
    SetupRequest, SetupSsoRequest, SetupStatus, SsoConfig, SsoProvider, StorageBackend,
    StorageMigrationResult, StorageSettings, UpdateDataPolicyRequest, UpdateInstanceRequest,
    UpdateNetworkRequest, UpdateSsoRequest, UpdateStorageRequest,
};
pub use pricing::{
    normalize_model_key, price_components, price_model_call, price_model_call_tiered,
    rate_from_usd_per_million, resolve_rates, CostComponents, ModelPricing, ModelRates, PricedCost,
    PricingBasis, RateTier, ServiceTierRates, TokenUsage, UnpricedReason,
};
pub use resource::{
    CreateDraftFileRequest, CreateResourceRequest, DeleteDraftEntryRequest,
    DeprecateResourceVersionRequest, DiagnosticSeverity, DraftFile, DraftFileTree,
    EffectiveResourceVersion, FileManifestEntry, ManagedResource, MoveDraftEntryRequest,
    ReleaseChannel, ReleaseResourceRequest, ReleaseResourceResult, ResourceAccessPolicy,
    ResourceBundle, ResourceBundleKind, ResourceChange, ResourceChangePage, ResourceCounts,
    ResourceDailyUsage, ResourceDiagnostic, ResourceFeedback, ResourceFetchCommit,
    ResourceFetchEntry, ResourceFetchHave, ResourceFetchObject, ResourceFetchRequest,
    ResourceFetchResponse, ResourceFetchTombstone, ResourceInstallationState,
    ResourceInventoryItem, ResourceInventoryMonitoring, ResourceInventoryMonitoringSummary,
    ResourceInventoryObservedState, ResourceInventoryRequest, ResourceInventoryResponse,
    ResourceKind, ResourceMemberUsage, ResourceMonitoring, ResourceMonitoringSummary,
    ResourceStatus, ResourceTargetMode, ResourceUsageBatchRequest, ResourceUsageBatchResponse,
    ResourceUsageEventRequest, ResourceUsageRejection, ResourceValidation, ResourceVersion,
    ResourceVersionLifecycleAction, ResourceVersionNotice, ResourceVersionStatus,
    ResourceVisibility, RestoreResourceVersionRequest, SaveDraftFileRequest, TeamManifest,
    UpdateResourceRequest, UpsertResourceFeedbackRequest, VersionMode, TEAM_AGENT_DIR,
};
pub use resource_semver::{SemanticVersion, SemanticVersionError};
pub use role::{
    CreateSubRoleRequest, CreateTagRequest, PrimaryRole, SubRole, Tag, UpdateSubRoleRequest,
    UpdateTagRequest,
};
pub use secret::{ConnectionSecret, CreateSecretRequest, CreatedSecret, SecretScope};
pub use spend::{
    evaluate_limit, validate_warn_percent, LimitEvaluation, LimitPeriod, LimitScope, LimitState,
    SpendLimitListResponse, SpendLimitPeriodStatus, SpendLimitSelector, SpendLimitView,
    UpsertSpendLimitRequest,
};
pub use telemetry::{
    DailyTokenUsage, MemberActivityItem, MemberActivityResponse, MemberCostReport,
    MemberCostReportRow, MemberPresence, MemberRequestDetail, MemberUsageSummary, ModelCostReport,
    ModelCostReportRow, ModelCostReportTotals, ModelUsageBreakdown, ResourceUsageActivityItem,
    ResourceUsageAnalytics, ResourceUsageBreakdown, ResourceUsageDay, ResourceUsageMember,
    ResourceUsageModel, ResourceUsageRole, ResourceUsageScope, ResourceUsageTotals,
    TelemetryBatchRequest, TelemetryBatchResponse, TelemetryDeliverySummary, TelemetryEventDetail,
    TelemetryEventRequest, TelemetryEventStatus, TelemetryEventType,
    TelemetryResourceAttributionDetail, TelemetryResourceRef, TelemetryResourceRelation,
    TelemetrySnapshot, UsageCoverage, UsageCoverageState, RETIRED_TELEMETRY_EVENT_FIELDS,
    UNKNOWN_TELEMETRY_LABEL,
};
pub use user::{
    ApproveMemberRequest, AuthSession, ChangePasswordRequest, CreateMemberRequest, CreatedMember,
    MemberListQuery, MemberListResponse, ResetPasswordResponse, UpdateMemberRequest, User,
    UserStatus,
};
