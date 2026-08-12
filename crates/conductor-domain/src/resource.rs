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
    Aim,
}

impl ResourceTargetMode {
    pub const ALL: [Self; 3] = [Self::Work, Self::Coding, Self::Aim];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Coding => "coding",
            Self::Aim => "aim",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "work" => Some(Self::Work),
            "coding" => Some(Self::Coding),
            "aim" => Some(Self::Aim),
            _ => None,
        }
    }
}

/// Resource kinds supported by the portable EvoFlux bundle contract.
///
/// Workflow and Command remain governed Conductor resources, but they are not
/// executable catalog bundles and therefore cannot be represented by this
/// deliberately narrower wire type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceBundleKind {
    Agent,
    Skill,
    Plugin,
}

impl ResourceBundleKind {
    pub fn from_resource_kind(kind: ResourceKind) -> Option<Self> {
        match kind {
            ResourceKind::Agent => Some(Self::Agent),
            ResourceKind::Skill => Some(Self::Skill),
            ResourceKind::Plugin => Some(Self::Plugin),
            ResourceKind::Workflow | ResourceKind::Command => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Skill => "skill",
            Self::Plugin => "plugin",
        }
    }
}

/// One immutable file in a [`ResourceBundle`].
///
/// `sha256` is the lowercase hex digest of the exact file bytes. The current
/// Conductor authoring pipeline accepts UTF-8 text and therefore emits
/// `executable = false`; keeping the field explicit prevents a future archive
/// importer from silently inventing execution permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileManifestEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub media_type: String,
    pub executable: bool,
}

/// Canonical, content-addressed descriptor for an EvoFlux Agent, Skill or
/// Plugin release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceBundle {
    pub schema_version: u8,
    pub kind: ResourceBundleKind,
    pub slug: String,
    pub version: String,
    pub artifact_sha256: String,
    pub artifact_size: u64,
    pub artifact_media_type: String,
    pub tree_sha256: String,
    pub files: Vec<FileManifestEntry>,
}

impl ResourceBundle {
    pub const SCHEMA_VERSION: u8 = 2;
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
    /// Additive bundle metadata. It is absent for legacy versions and for
    /// governed resource kinds outside the Agent/Skill/Plugin bundle contract.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "bundle_v2")]
    pub bundle: Option<ResourceBundle>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_schema_version: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_count: Option<u32>,
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

/// One resource/version the client already has in its managed checkout.
/// This is the equivalent of Git's `have` negotiation: Conductor uses it to
/// return only changed tree entries and missing immutable objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFetchHave {
    pub resource_id: Uuid,
    pub version_id: Uuid,
    pub artifact_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFetchRequest {
    pub installation_id: Uuid,
    pub have_commit: Option<String>,
    #[serde(default)]
    pub have: Vec<ResourceFetchHave>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFetchCommit {
    /// Content identity of the complete desired resource tree.
    pub id: String,
    pub tree_sha256: String,
    /// Durable project change watermark observed while building the tree.
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFetchEntry {
    pub resource_id: Uuid,
    pub version_id: Uuid,
    pub kind: ResourceBundleKind,
    pub slug: String,
    pub version: String,
    pub release_channel: ReleaseChannel,
    pub bundle: ResourceBundle,
    pub minimum_evoflux_version: Option<String>,
    pub trust_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFetchTombstone {
    pub resource_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFetchObject {
    pub artifact_sha256: String,
    pub size: u64,
    pub media_type: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFetchResponse {
    pub schema_version: u8,
    pub project_id: Uuid,
    pub base_commit: Option<String>,
    pub commit: ResourceFetchCommit,
    pub up_to_date: bool,
    pub entries: Vec<ResourceFetchEntry>,
    pub tombstones: Vec<ResourceFetchTombstone>,
    pub objects: Vec<ResourceFetchObject>,
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
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "bundle_v2")]
    pub bundle: Option<ResourceBundle>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_mode_round_trips_aim() {
        assert_eq!(
            ResourceTargetMode::parse("aim"),
            Some(ResourceTargetMode::Aim)
        );
        assert_eq!(ResourceTargetMode::Aim.as_str(), "aim");
        assert_eq!(
            serde_json::to_value(ResourceTargetMode::Aim).unwrap(),
            serde_json::json!("aim")
        );
    }

    #[test]
    fn bundle_has_a_stable_wire_shape() {
        let bundle = ResourceBundle {
            schema_version: ResourceBundle::SCHEMA_VERSION,
            kind: ResourceBundleKind::Agent,
            slug: "reviewer".into(),
            version: "1.2.3".into(),
            artifact_sha256: "a".repeat(64),
            artifact_size: 42,
            artifact_media_type: "application/vnd.evoflux.resource+json".into(),
            tree_sha256: "b".repeat(64),
            files: vec![FileManifestEntry {
                path: "reviewer.md".into(),
                sha256: "c".repeat(64),
                size: 12,
                media_type: "text/markdown".into(),
                executable: false,
            }],
        };

        let value = serde_json::to_value(&bundle).unwrap();
        assert_eq!(value["schema_version"], 2);
        assert_eq!(value["kind"], "agent");
        assert_eq!(value["artifact_sha256"], "a".repeat(64));
        assert_eq!(value["tree_sha256"], "b".repeat(64));
        assert_eq!(value["files"][0]["media_type"], "text/markdown");
        assert_eq!(value["files"][0]["executable"], false);
        assert_eq!(
            serde_json::from_value::<ResourceBundle>(value).unwrap(),
            bundle
        );
    }

    #[test]
    fn legacy_bundle_field_is_read_but_canonical_output_uses_bundle() {
        let version: EffectiveResourceVersion = serde_json::from_value(serde_json::json!({
            "project_id": Uuid::nil(),
            "resource_id": Uuid::nil(),
            "version_id": Uuid::nil(),
            "kind": "skill",
            "slug": "audit",
            "version": "1.0.0",
            "description": null,
            "changelog": null,
            "version_history": [],
            "release_channel": "published",
            "payload": {},
            "sha256": "a".repeat(64),
            "size": 42,
            "artifact_key": null,
            "bundle_v2": {
                "schema_version": 2,
                "kind": "skill",
                "slug": "audit",
                "version": "1.0.0",
                "artifact_sha256": "a".repeat(64),
                "artifact_size": 42,
                "artifact_media_type": "application/vnd.evoflux.resource+zip",
                "tree_sha256": "b".repeat(64),
                "files": []
            },
            "minimum_evoflux_version": null
        }))
        .unwrap();

        assert!(version.bundle.is_some());
        let canonical = serde_json::to_value(version).unwrap();
        assert!(canonical.get("bundle").is_some());
        assert!(canonical.get("bundle_v2").is_none());
    }

    #[test]
    fn legacy_change_without_bundle_fields_still_deserializes() {
        let change: ResourceChange = serde_json::from_value(serde_json::json!({
            "project_id": Uuid::nil(),
            "resource_id": Uuid::nil(),
            "version_id": null,
            "kind": "skill",
            "slug": "audit",
            "version": null,
            "description": null,
            "changelog": null,
            "release_channel": null,
            "sha256": null,
            "size": 0,
            "minimum_evoflux_version": null,
            "trust_required": false,
            "tombstone": true
        }))
        .unwrap();

        assert!(change.version_history.is_empty());
        assert_eq!(change.bundle_schema_version, None);
        assert_eq!(change.artifact_sha256, None);
        assert_eq!(change.tree_sha256, None);
        assert_eq!(change.file_count, None);
        let serialized = serde_json::to_value(change).unwrap();
        assert!(serialized.get("bundle_schema_version").is_none());
        assert!(serialized.get("artifact_sha256").is_none());
    }

    #[test]
    fn smart_fetch_have_contract_is_strict_and_content_addressed() {
        let resource_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        let request: ResourceFetchRequest = serde_json::from_value(serde_json::json!({
            "installation_id": Uuid::new_v4(),
            "have_commit": "a".repeat(64),
            "have": [{
                "resource_id": resource_id,
                "version_id": version_id,
                "artifact_sha256": "b".repeat(64)
            }]
        }))
        .unwrap();

        assert_eq!(request.have.len(), 1);
        assert_eq!(request.have[0].resource_id, resource_id);
        assert_eq!(request.have[0].version_id, version_id);
        assert_eq!(request.have[0].artifact_sha256, "b".repeat(64));
    }
}
