use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::role::{PrimaryRole, SubRole};

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
