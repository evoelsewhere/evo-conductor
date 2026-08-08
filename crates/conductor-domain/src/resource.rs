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
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceVisibility {
    Shared,
    Private,
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
