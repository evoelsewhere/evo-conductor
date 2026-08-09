use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Agent,
    Skill,
    Mcp,
    Workflow,
    Command,
}

impl ResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Skill => "skill",
            Self::Mcp => "mcp",
            Self::Workflow => "workflow",
            Self::Command => "command",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "agent" => Some(Self::Agent),
            "skill" => Some(Self::Skill),
            "mcp" => Some(Self::Mcp),
            "workflow" => Some(Self::Workflow),
            "command" => Some(Self::Command),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedResource {
    pub id: Uuid,
    pub kind: ResourceKind,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
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
    Published,
    Archived,
}

impl ResourceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
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
    Published,
    Deprecated,
}

impl ResourceVersionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Deprecated => "deprecated",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "published" => Self::Published,
            "deprecated" => Self::Deprecated,
            _ => Self::Draft,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceVersion {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub version: String,
    pub status: ResourceVersionStatus,
    pub payload: serde_json::Value,
    pub changelog: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResourceRequest {
    pub kind: ResourceKind,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub visibility: ResourceVisibility,
    #[serde(default)]
    pub payload: serde_json::Value,
    pub changelog: Option<String>,
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
    pub mcp: u32,
    pub workflows: u32,
}
