use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::role::PrimaryRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Agent,
    Skill,
    Plugin,
    Workflow,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceTargetMode {
    Work,
    Coding,
}

impl ResourceTargetMode {
    pub const ALL: [Self; 2] = [Self::Work, Self::Coding];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Coding => "coding",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "work" => Some(Self::Work),
            "coding" => Some(Self::Coding),
            _ => None,
        }
    }
}

impl ResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Skill => "skill",
            Self::Plugin => "plugin",
            Self::Workflow => "workflow",
            Self::Command => "command",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "agent" => Some(Self::Agent),
            "skill" => Some(Self::Skill),
            "plugin" | "mcp" => Some(Self::Plugin),
            "workflow" => Some(Self::Workflow),
            "command" => Some(Self::Command),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedResource {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: ResourceKind,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub highest_version: Option<String>,
    pub draft_revision: u64,
    pub release_channel: Option<ReleaseChannel>,
    pub owner_user_id: Option<Uuid>,
    pub visibility: ResourceVisibility,
    pub status: ResourceStatus,
    pub payload: serde_json::Value,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceVisibility {
    Shared,
    Private,
}

impl ResourceVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Private => "private",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceStatus {
    Draft,
    Beta,
    Published,
    Archived,
}

impl ResourceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Beta => "beta",
            Self::Published => "published",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "beta" => Self::Beta,
            "published" => Self::Published,
            "archived" => Self::Archived,
            _ => Self::Draft,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceVersionStatus {
    Draft,
    Beta,
    Published,
    Deprecated,
}

impl ResourceVersionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Beta => "beta",
            Self::Published => "published",
            Self::Deprecated => "deprecated",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "beta" => Self::Beta,
            "published" => Self::Published,
            "deprecated" => Self::Deprecated,
            _ => Self::Draft,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceVersion {
    pub id: Uuid,
    pub project_id: Uuid,
    pub resource_id: Uuid,
    pub version: String,
    pub status: ResourceVersionStatus,
    pub payload: serde_json::Value,
    pub changelog: Option<String>,
    pub release_channel: Option<ReleaseChannel>,
    pub active_channel: Option<ReleaseChannel>,
    pub content_sha256: String,
    pub content_size: u64,
    pub artifact_key: Option<String>,
    pub minimum_evoflux_version: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub deprecated_at: Option<DateTime<Utc>>,
    pub deprecated_by: Option<Uuid>,
    pub deprecation_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceVersionNotice {
    pub version_id: Uuid,
    pub version: String,
    pub status: ResourceVersionStatus,
    pub release_channel: ReleaseChannel,
    pub changelog: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub deprecation_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecateResourceVersionRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResourceVersionRequest {
    pub draft_revision: u64,
    #[serde(default)]
    pub confirm_deprecated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceVersionLifecycleAction {
    Deprecate,
    RestoreToDraft,
}

impl ResourceVersionLifecycleAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deprecate => "deprecate",
            Self::RestoreToDraft => "restore_to_draft",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResourceRequest {
    pub kind: ResourceKind,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_initial_version")]
    pub version: String,
    pub visibility: ResourceVisibility,
    #[serde(default)]
    pub payload: serde_json::Value,
    pub changelog: Option<String>,
}

fn default_initial_version() -> String {
    "0.1.0".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    Beta,
    Published,
}

impl ReleaseChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Beta => "beta",
            Self::Published => "published",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "beta" => Some(Self::Beta),
            "published" => Some(Self::Published),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VersionMode {
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseResourceRequest {
    pub channel: ReleaseChannel,
    #[serde(default)]
    pub version_mode: VersionMode,
    pub manual_version: Option<String>,
    pub draft_revision: u64,
    pub changelog: Option<String>,
    #[serde(default)]
    pub beta_member_ids: Vec<Uuid>,
    pub minimum_evoflux_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseResourceResult {
    pub resource_id: Uuid,
    pub version_id: Uuid,
    pub version: String,
    pub channel: ReleaseChannel,
    pub sha256: String,
    pub size: u64,
    pub highest_version: String,
    pub next_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftFileTree {
    pub resource_id: Uuid,
    pub revision: u64,
    pub files: Vec<DraftFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDraftFileRequest {
    pub content: String,
    pub draft_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDraftFileRequest {
    pub path: String,
    #[serde(default)]
    pub content: String,
    pub draft_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveDraftEntryRequest {
    pub path: String,
    pub destination_path: String,
    pub draft_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteDraftEntryRequest {
    pub path: String,
    pub draft_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceValidation {
    pub valid: bool,
    pub revision: u64,
    pub diagnostics: Vec<ResourceDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChange {
    pub project_id: Uuid,
    pub resource_id: Uuid,
    pub version_id: Option<Uuid>,
    pub kind: ResourceKind,
    pub slug: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub changelog: Option<String>,
    #[serde(default)]
    pub version_history: Vec<ResourceVersionNotice>,
    pub release_channel: Option<ReleaseChannel>,
    pub sha256: Option<String>,
    pub size: u64,
    pub minimum_evoflux_version: Option<String>,
    pub trust_required: bool,
    pub tombstone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChangePage {
    pub schema_version: u8,
    pub project_id: Uuid,
    pub next_cursor: String,
    pub has_more: bool,
    pub changes: Vec<ResourceChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveResourceVersion {
    pub project_id: Uuid,
    pub resource_id: Uuid,
    pub version_id: Uuid,
    pub kind: ResourceKind,
    pub slug: String,
    pub version: String,
    pub description: Option<String>,
    pub changelog: Option<String>,
    #[serde(default)]
    pub version_history: Vec<ResourceVersionNotice>,
    pub release_channel: ReleaseChannel,
    pub payload: serde_json::Value,
    pub sha256: String,
    pub size: u64,
    pub artifact_key: Option<String>,
    pub minimum_evoflux_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInventoryItem {
    pub resource_id: Uuid,
    pub desired_version_id: Option<Uuid>,
    pub applied_version_id: Option<Uuid>,
    pub release_channel: Option<ReleaseChannel>,
    pub content_sha256: Option<String>,
    pub plugin_installation_id: Option<String>,
    pub observed_state: String,
    pub error_category: Option<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceInventoryObservedState {
    Pending,
    Staged,
    TrustPending,
    UpdatePending,
    Applied,
    InSync,
    Declined,
    Incompatible,
    OwnershipConflict,
    ProjectScopeMismatch,
    Error,
    Removed,
}

impl ResourceInventoryObservedState {
    pub const INSTALLED: [Self; 2] = [Self::Applied, Self::InSync];
    pub const PENDING: [Self; 4] = [
        Self::Pending,
        Self::Staged,
        Self::TrustPending,
        Self::UpdatePending,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Staged => "staged",
            Self::TrustPending => "trust_pending",
            Self::UpdatePending => "update_pending",
            Self::Applied => "applied",
            Self::InSync => "in_sync",
            Self::Declined => "declined",
            Self::Incompatible => "incompatible",
            Self::OwnershipConflict => "ownership_conflict",
            Self::ProjectScopeMismatch => "project_scope_mismatch",
            Self::Error => "error",
            Self::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInventoryRequest {
    pub installation_id: Uuid,
    pub items: Vec<ResourceInventoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInventoryResponse {
    pub accepted: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceInventoryMonitoringSummary {
    pub reported_installations: u64,
    pub installed_installations: u64,
    pub installed_members: u64,
    pub pending_installations: u64,
    pub attention_installations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInstallationState {
    pub installation_id: Uuid,
    pub installation_name: String,
    pub platform: String,
    pub evoflux_version: String,
    pub user_id: Uuid,
    pub member_name: String,
    pub email: String,
    pub primary_role: PrimaryRole,
    pub desired_version_id: Option<Uuid>,
    pub desired_version: Option<String>,
    pub applied_version_id: Option<Uuid>,
    pub applied_version: Option<String>,
    pub release_channel: Option<ReleaseChannel>,
    pub plugin_installation_id: Option<String>,
    pub observed_state: String,
    pub error_category: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInventoryMonitoring {
    pub resource_id: Uuid,
    pub summary: ResourceInventoryMonitoringSummary,
    pub installations: Vec<ResourceInstallationState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResourceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub visibility: Option<ResourceVisibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResourceVersionRequest {
    pub version: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    pub changelog: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ResourceAccessPolicy {
    pub all_members: bool,
    pub primary_roles: Vec<String>,
    pub sub_role_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub member_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFeedback {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub resource_version: String,
    pub user_id: Uuid,
    pub member_name: String,
    pub rating: u8,
    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertResourceFeedbackRequest {
    pub rating: u8,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceUsageOutcome {
    Success,
    Failure,
    Cancelled,
}

impl ResourceUsageOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageEventRequest {
    pub event_id: Uuid,
    pub resource_id: Uuid,
    pub resource_version: String,
    pub session_id: Option<String>,
    pub outcome: ResourceUsageOutcome,
    pub duration_ms: u64,
    #[serde(default)]
    pub tokens_in: u64,
    #[serde(default)]
    pub tokens_out: u64,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageBatchRequest {
    pub events: Vec<ResourceUsageEventRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageBatchResponse {
    pub accepted: u32,
    pub duplicates: u32,
    pub rejected: u32,
    pub rejections: Vec<ResourceUsageRejection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageRejection {
    pub event_id: Uuid,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceMonitoringSummary {
    pub executions: u64,
    pub successes: u64,
    pub failures: u64,
    pub active_members: u32,
    pub success_rate: f64,
    pub average_duration_ms: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub feedback_count: u32,
    pub average_rating: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDailyUsage {
    pub date: String,
    pub executions: u64,
    pub successes: u64,
    pub failures: u64,
    pub average_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMemberUsage {
    pub user_id: Uuid,
    pub member_name: String,
    pub executions: u64,
    pub success_rate: f64,
    pub average_duration_ms: u64,
    pub last_used_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMonitoring {
    pub resource_id: Uuid,
    pub days: u32,
    pub summary: ResourceMonitoringSummary,
    pub daily: Vec<ResourceDailyUsage>,
    pub members: Vec<ResourceMemberUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub project_name: String,
    pub members_total: u32,
    pub members_online: u32,
    pub secrets_active: u32,
    pub resources: ResourceCounts,
    pub sso_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceCounts {
    pub agents: u32,
    pub skills: u32,
    pub plugins: u32,
    pub workflows: u32,
}
