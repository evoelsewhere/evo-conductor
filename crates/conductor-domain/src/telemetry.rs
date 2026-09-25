use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::resource::ResourceKind;
use crate::role::{PrimaryRole, SubRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventType {
    Request,
    ModelCall,
    ToolCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventStatus {
    Success,
    Error,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceUsageScope {
    All,
    #[default]
    Governed,
}

impl TelemetryEventStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "success" => Some(Self::Success),
            "error" => Some(Self::Error),
            "blocked" => Some(Self::Blocked),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

pub const UNKNOWN_TELEMETRY_LABEL: &str = "unknown";

impl TelemetryEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::ModelCall => "model_call",
            Self::ToolCall => "tool_call",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "request" => Some(Self::Request),
            "model_call" => Some(Self::ModelCall),
            "tool_call" => Some(Self::ToolCall),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryResourceRelation {
    ExecutingAgent,
    ActivatedSkill,
    PluginContributedSkill,
    PluginContributedTool,
}

impl TelemetryResourceRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExecutingAgent => "executing_agent",
            Self::ActivatedSkill => "activated_skill",
            Self::PluginContributedSkill => "plugin_contributed_skill",
            Self::PluginContributedTool => "plugin_contributed_tool",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "executing_agent" => Some(Self::ExecutingAgent),
            "activated_skill" => Some(Self::ActivatedSkill),
            "plugin_contributed_skill" => Some(Self::PluginContributedSkill),
            "plugin_contributed_tool" => Some(Self::PluginContributedTool),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryResourceRef {
    pub resource_id: Uuid,
    pub version_id: Uuid,
    pub relation: TelemetryResourceRelation,
    pub plugin_installation_id: Option<String>,
}

/// Fields this contract used to carry. They are tolerated so an installation
/// on an older build keeps reporting: rejecting the batch would take the
/// whole fleet's telemetry offline until the last client upgraded. They are
/// read from the wire and discarded.
///
/// This list is exhaustive on purpose. Anything else unknown is still
/// refused, because `deny_unknown_fields` was doing two jobs here — pinning
/// the contract, and refusing a payload that carries conversation content.
/// Only the first job is being relaxed.
pub const RETIRED_TELEMETRY_EVENT_FIELDS: [&str; 5] = [
    "tool_name",
    "tool_category",
    "estimated_cost_usd_micros",
    "cost_source",
    "evoflux_version",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEventRequest {
    pub event_id: Uuid,
    pub request_id: String,
    pub session_id: Option<String>,
    pub event_type: TelemetryEventType,
    #[serde(default)]
    pub sequence: u32,
    pub agent_name: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub response_model: Option<String>,
    #[serde(default)]
    pub tokens_in: u64,
    #[serde(default)]
    pub tokens_out: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    /// Cache-*write* tokens, which providers bill above ordinary input. Sent
    /// separately because they are already inside `tokens_in`, and Conductor
    /// cannot price them correctly without knowing how many there were.
    #[serde(default)]
    pub cache_write_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(default)]
    pub tool_use_tokens: u64,
    #[serde(default)]
    pub duration_ms: u64,
    pub status: TelemetryEventStatus,
    pub error_category: Option<String>,
    /// Provider service tier the call was billed under. Priced: a lane that
    /// publishes its own rates overrides the model's headline price.
    pub service_tier: Option<String>,
    #[serde(default)]
    pub resources: Vec<TelemetryResourceRef>,
    pub reported_at: DateTime<Utc>,
    /// Everything the contract does not name. Retired fields are allowed
    /// through here and ignored; anything else is refused at validation, so a
    /// client cannot smuggle conversation content into an event by inventing
    /// a key for it.
    #[serde(flatten)]
    pub unknown: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryBatchRequest {
    pub installation_id: Uuid,
    pub events: Vec<TelemetryEventRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryBatchResponse {
    pub accepted: u32,
    pub duplicates: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<TelemetryDeliverySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryDeliverySummary {
    pub installation_id: Uuid,
    pub window_days: u16,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub events: u64,
    pub requests: u64,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub estimated_cost_usd_micros: u64,
    pub unpriced_model_calls: u64,
    pub attributed_events: u64,
    pub attributed_requests: u64,
    pub attributed_model_calls: u64,
    pub attributed_tool_calls: u64,
    pub attributed_estimated_cost_usd_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsageBreakdown {
    pub provider: String,
    pub model: String,
    pub calls: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub total_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub estimated_cost_usd_micros: u64,
    /// How much cheaper this model's cache reads were than paying full
    /// input price for the same tokens.
    pub cache_savings_usd_micros: u64,
    pub unpriced_calls: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTokenUsage {
    pub date: String,
    pub requests: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberUsageSummary {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub total_requests: u64,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub error_count: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub total_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    /// Same authoritative cost the project analytics report, over the same
    /// `received_at` window, so a member's own page and the admin analytics
    /// agree for a given range.
    pub estimated_cost_usd_micros: u64,
    /// How much cheaper this member's cache reads were than paying full
    /// input price for the same tokens, over the same window.
    pub cache_savings_usd_micros: u64,
    pub unpriced_model_calls: u64,
    pub models: Vec<ModelUsageBreakdown>,
    pub daily: Vec<DailyTokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberActivityItem {
    pub request_id: String,
    pub session_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub total_tokens: u64,
    pub duration_ms: u64,
    pub estimated_cost_usd_micros: u64,
    pub unpriced_model_calls: u64,
    pub status: TelemetryEventStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberActivityResponse {
    pub items: Vec<MemberActivityItem>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEventDetail {
    pub event_id: Uuid,
    pub event_type: TelemetryEventType,
    pub sequence: u32,
    pub agent_name: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub response_model: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub reasoning_tokens: u64,
    pub tool_use_tokens: u64,
    pub duration_ms: u64,
    pub status: TelemetryEventStatus,
    pub error_category: Option<String>,
    /// Conductor's own price for this event, absent when it could not price
    /// it. There is no second opinion to fall back on: clients report usage,
    /// never cost.
    pub estimated_cost_usd_micros: Option<u64>,
    pub resources: Vec<TelemetryResourceAttributionDetail>,
    pub reported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryResourceAttributionDetail {
    pub resource_id: Uuid,
    pub version_id: Uuid,
    pub kind: ResourceKind,
    pub name: String,
    pub version: String,
    pub relation: TelemetryResourceRelation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRequestDetail {
    pub request: MemberActivityItem,
    pub events: Vec<TelemetryEventDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberPresence {
    pub user_id: Uuid,
    pub display_name: String,
    pub primary_role: PrimaryRole,
    pub sub_roles: Vec<SubRole>,
    pub evoflux_connected: bool,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub agents_count: u32,
    pub skills_count: u32,
    pub mcp_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub user_id: Uuid,
    pub session_id: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub tool_calls: u64,
    pub active_agents: u32,
    pub reported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageTotals {
    /// Current inventory rows reported by connected EvoFlux installations.
    pub reported_installations: u64,
    pub installed_installations: u64,
    pub installed_members: u64,
    pub pending_installations: u64,
    pub attention_installations: u64,
    /// Distinct EvoFlux request identities received in the selected range,
    /// whether or not a governed resource was attributed to them. Project,
    /// member, role, installation, outcome, model and tool facets apply;
    /// resource-specific facets intentionally do not narrow this denominator.
    pub all_requests: u64,
    /// Distinct request identities attributed to at least one governed
    /// resource. This comparison count is returned for both scope views so
    /// clients can explain attribution coverage without mixing aggregates.
    pub governed_requests: u64,
    /// Distinct request identities in the selected scope and matching every
    /// supported filter. This equals `all_requests` for `scope=all` and the
    /// governed subset for `scope=governed`.
    pub requests: u64,
    pub resource_uses: u64,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub successes: u64,
    pub errors: u64,
    pub blocked: u64,
    pub cancelled: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub tool_use_tokens: u64,
    pub total_tokens: u64,
    /// Conductor's own price for the window. There is exactly one costing
    /// method behind this number: clients report usage, never cost, so a
    /// figure is either Conductor-priced or absent. What is absent shows up
    /// in `unpriced_model_calls` rather than being filled in from elsewhere.
    pub estimated_cost_usd_micros: u64,
    /// How much cheaper this window's cache reads were than paying full
    /// input price for the same tokens.
    pub cache_savings_usd_micros: u64,
    /// Model calls Conductor could not price. Volume this number covers is
    /// missing from `estimated_cost_usd_micros`, not free.
    pub unpriced_model_calls: u64,
    pub average_tokens_per_request: u64,
    pub average_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageDay {
    pub date: String,
    pub requests: u64,
    pub successes: u64,
    pub errors: u64,
    pub blocked: u64,
    pub cancelled: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read_tokens: u64,
    pub reasoning_tokens: u64,
    pub tool_use_tokens: u64,
    pub estimated_cost_usd_micros: u64,
    pub unpriced_model_calls: u64,
}

/// One row per (resource version, attribution relation).
///
/// Every counter here describes **the requests that used this version**, not
/// this version's share of them: a request attributed to several resources is
/// counted in full against each one. Rows therefore overlap and do not add up
/// to [`ResourceUsageTotals`] — they exist to rank versions against each
/// other, not to partition a total. `estimated_cost_usd_micros` is the one
/// most likely to be summed by mistake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageBreakdown {
    pub resource_id: Uuid,
    pub version_id: Uuid,
    pub kind: ResourceKind,
    pub name: String,
    pub version: String,
    pub relation: TelemetryResourceRelation,
    pub uses: u64,
    pub members: u64,
    pub requests: u64,
    pub successes: u64,
    pub errors: u64,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd_micros: u64,
    pub last_used_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageMember {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub primary_role: PrimaryRole,
    pub requests: u64,
    pub resource_uses: u64,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub installations: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd_micros: u64,
    pub last_received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageModel {
    pub provider: String,
    pub model: String,
    pub calls: u64,
    pub total_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub estimated_cost_usd_micros: u64,
    /// How much cheaper this model's cache reads were than paying full
    /// input price for the same tokens.
    pub cache_savings_usd_micros: u64,
    pub unpriced_calls: u64,
}

/// One model's spend for a project, broken into the same components a
/// models.dev rate prices separately.
///
/// `total_cost_usd_micros` is the same authoritative figure every other
/// project view reports (Conductor's price where it has one, the client's
/// otherwise). The four component costs are a proportional split of that
/// same total against the current rate card, so they always sum to it
/// exactly -- they are not independently re-derived and can shift slightly if
/// a rate changed since the events in this row were priced.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCostReportRow {
    pub provider: String,
    pub model: String,
    pub calls: u64,
    /// Model calls this project has no server-computed price for. Excluded
    /// from every cost figure below; present so unpriced volume stays visible
    /// rather than silently reading as free.
    pub unpriced_calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub input_cost_usd_micros: u64,
    pub output_cost_usd_micros: u64,
    pub cache_read_cost_usd_micros: u64,
    pub cache_write_cost_usd_micros: u64,
    pub total_cost_usd_micros: u64,
    /// How much cheaper this row's cache reads were than paying full input
    /// price for the same tokens -- a separate figure from
    /// `cache_read_cost_usd_micros`, which is what the cache reads
    /// themselves cost, not what they saved against the alternative.
    pub cache_savings_usd_micros: u64,
    /// Blended cost per million tokens: `total_cost_usd_micros / total_tokens
    /// * 1e6`. Zero when `total_tokens` is zero.
    pub avg_usd_micros_per_million_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCostReportTotals {
    pub calls: u64,
    pub unpriced_calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub total_cost_usd_micros: u64,
    pub cache_savings_usd_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCostReport {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub rows: Vec<ModelCostReportRow>,
    pub totals: ModelCostReportTotals,
}

/// One member's spend for a project: the same window and figures as
/// `ModelCostReportRow`, grouped by who spent rather than what was spent on.
/// A member's calls can span many models with different rates, so unlike the
/// model report this has no per-component cost split -- only the token
/// components and the one authoritative total.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberCostReportRow {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub primary_role: PrimaryRole,
    pub calls: u64,
    pub unpriced_calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub total_cost_usd_micros: u64,
    pub cache_savings_usd_micros: u64,
    /// Blended cost per million tokens: `total_cost_usd_micros / total_tokens
    /// * 1e6`. Zero when `total_tokens` is zero.
    pub avg_usd_micros_per_million_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberCostReport {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub rows: Vec<MemberCostReportRow>,
    pub totals: ModelCostReportTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageRole {
    pub primary_role: PrimaryRole,
    pub requests: u64,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd_micros: u64,
}

/// One row per (request, resource version, attribution relation).
///
/// A request that used several resources appears once per attribution, and
/// each of those rows carries the whole request's calls, tokens and cost
/// rather than a share of them. Summing a column across rows counts such a
/// request more than once — the same overlap as [`ResourceUsageBreakdown`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageActivityItem {
    pub request_id: String,
    pub user_id: Uuid,
    pub display_name: String,
    pub primary_role: PrimaryRole,
    pub resource_id: Uuid,
    pub version_id: Uuid,
    pub kind: ResourceKind,
    pub resource_name: String,
    pub version: String,
    pub relation: TelemetryResourceRelation,
    pub occurred_at: DateTime<Utc>,
    pub status: TelemetryEventStatus,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd_micros: u64,
    pub unpriced_model_calls: u64,
    pub duration_ms: u64,
}

/// Whether the figures in a report rest on a complete set of reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageCoverageState {
    /// Every installation was in contact on every closed day it existed for.
    Complete,
    /// At least one installation was silent for at least one closed day, so
    /// the totals are a floor rather than the whole picture.
    Incomplete,
    /// The window contains no closed day yet, so there is nothing to judge.
    Unknown,
}

impl UsageCoverageState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
            Self::Unknown => "unknown",
        }
    }
}

/// How much of a reporting window the fleet actually accounted for.
///
/// This exists to separate two things that otherwise look identical in a
/// spend report: an installation that was running and genuinely used nothing,
/// and an installation that never checked in at all. Only the second is
/// missing data, and only the second means a total is understated.
///
/// Numbers are still reported when coverage is incomplete — hiding them would
/// trade a known understatement for no information at all. The state is the
/// caveat that has to travel with them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCoverage {
    pub state: UsageCoverageState,
    /// Last fully-elapsed UTC day inside the window. Coverage is judged only
    /// up to here: an installation cannot be faulted for not yet having
    /// reported a day that is still in progress.
    pub through_day: Option<String>,
    /// Installation-days the fleet was expected to account for — one per
    /// installation per closed day at or after it registered.
    pub expected_installation_days: u64,
    /// Of those, the ones with at least one event or heartbeat received.
    pub covered_installation_days: u64,
    pub missing_installation_days: u64,
    /// Installations that registered on or before the window but were never
    /// in contact during it. The loudest form of missing data.
    pub silent_installations: u64,
    /// Installations counted toward `expected_installation_days`.
    pub expected_installations: u64,
    /// Covered share in basis points, so the ratio survives JSON without
    /// picking up float drift. 10000 is complete.
    pub covered_bps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageAnalytics {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    /// Declares whether totals, daily rows, members, models, roles and tools
    /// represent every received project event or only governed-attributed
    /// events. Resources and request activity are available only in the
    /// governed view because they require a resource attribution.
    pub scope: ResourceUsageScope,
    /// Whether the fleet actually accounted for the window. Read this before
    /// the numbers: incomplete coverage makes every total below a floor.
    pub coverage: UsageCoverage,
    pub totals: ResourceUsageTotals,
    pub daily: Vec<ResourceUsageDay>,
    pub resources: Vec<ResourceUsageBreakdown>,
    pub members: Vec<ResourceUsageMember>,
    pub models: Vec<ResourceUsageModel>,
    pub roles: Vec<ResourceUsageRole>,
    pub activity: Vec<ResourceUsageActivityItem>,
    pub activity_total: u64,
    pub limit: u32,
    pub offset: u32,
}
