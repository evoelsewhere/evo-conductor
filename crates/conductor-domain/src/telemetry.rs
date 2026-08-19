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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryToolCategory {
    Mcp,
    Filesystem,
    Web,
    VersionControl,
    Collaboration,
    Other,
}

impl TelemetryToolCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mcp => "mcp",
            Self::Filesystem => "filesystem",
            Self::Web => "web",
            Self::VersionControl => "version_control",
            Self::Collaboration => "collaboration",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "mcp" => Some(Self::Mcp),
            "filesystem" => Some(Self::Filesystem),
            "web" => Some(Self::Web),
            "version_control" => Some(Self::VersionControl),
            "collaboration" => Some(Self::Collaboration),
            "other" => Some(Self::Other),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryCostSource {
    #[serde(rename = "evoflux_catalog")]
    EvoFluxCatalog,
}

impl TelemetryCostSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EvoFluxCatalog => "evoflux_catalog",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "evoflux_catalog" => Some(Self::EvoFluxCatalog),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(default)]
    pub tool_use_tokens: u64,
    #[serde(default)]
    pub duration_ms: u64,
    pub tool_name: Option<String>,
    pub tool_category: Option<TelemetryToolCategory>,
    pub status: TelemetryEventStatus,
    pub error_category: Option<String>,
    pub estimated_cost_usd_micros: Option<u64>,
    pub cost_source: Option<TelemetryCostSource>,
    pub evoflux_version: Option<String>,
    #[serde(default)]
    pub resources: Vec<TelemetryResourceRef>,
    pub reported_at: DateTime<Utc>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTokenUsage {
    pub date: String,
    pub requests: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub total_tokens: u64,
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
    pub reasoning_tokens: u64,
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
    pub tool_name: Option<String>,
    pub tool_category: Option<TelemetryToolCategory>,
    pub status: TelemetryEventStatus,
    pub error_category: Option<String>,
    pub estimated_cost_usd_micros: Option<u64>,
    pub cost_source: Option<TelemetryCostSource>,
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
pub struct MemberToolUsage {
    pub tool_name: String,
    pub category: TelemetryToolCategory,
    pub calls: u64,
    pub successes: u64,
    pub errors: u64,
    pub average_duration_ms: u64,
    pub last_used_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberToolsSummary {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub tools: Vec<MemberToolUsage>,
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
    pub reasoning_tokens: u64,
    pub tool_use_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd_micros: u64,
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
    pub estimated_cost_usd_micros: u64,
    pub unpriced_calls: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageTool {
    pub tool_name: String,
    pub category: TelemetryToolCategory,
    pub calls: u64,
    pub successes: u64,
    pub errors: u64,
    pub blocked: u64,
    pub cancelled: u64,
    pub average_duration_ms: u64,
    pub last_used_at: DateTime<Utc>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageAnalytics {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub totals: ResourceUsageTotals,
    pub daily: Vec<ResourceUsageDay>,
    pub resources: Vec<ResourceUsageBreakdown>,
    pub members: Vec<ResourceUsageMember>,
    pub models: Vec<ResourceUsageModel>,
    pub roles: Vec<ResourceUsageRole>,
    pub tools: Vec<ResourceUsageTool>,
    pub activity: Vec<ResourceUsageActivityItem>,
    pub activity_total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[cfg(test)]
mod tests {
    use super::{TelemetryEventStatus, TelemetryEventType, TelemetryToolCategory};

    #[test]
    fn telemetry_wire_enums_round_trip() {
        for event_type in [
            TelemetryEventType::Request,
            TelemetryEventType::ModelCall,
            TelemetryEventType::ToolCall,
        ] {
            assert_eq!(
                TelemetryEventType::parse(event_type.as_str()),
                Some(event_type)
            );
        }
        for status in [
            TelemetryEventStatus::Success,
            TelemetryEventStatus::Error,
            TelemetryEventStatus::Blocked,
            TelemetryEventStatus::Cancelled,
        ] {
            assert_eq!(TelemetryEventStatus::parse(status.as_str()), Some(status));
        }
        for category in [
            TelemetryToolCategory::Mcp,
            TelemetryToolCategory::Filesystem,
            TelemetryToolCategory::Web,
            TelemetryToolCategory::VersionControl,
            TelemetryToolCategory::Collaboration,
            TelemetryToolCategory::Other,
        ] {
            assert_eq!(
                TelemetryToolCategory::parse(category.as_str()),
                Some(category)
            );
        }
    }
}
