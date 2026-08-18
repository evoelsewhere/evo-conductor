use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ResourceCounts;

/// Three missed 60-second client-heartbeat intervals before presence is stale.
pub const DASHBOARD_PRESENCE_THRESHOLD_SECONDS: u32 = 180;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardPresence {
    pub clients_seen_recently: u32,
    pub members_seen_recently: u32,
    pub threshold_seconds: u32,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardRealtimeScope {
    ThisNode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardRealtime {
    pub scope: DashboardRealtimeScope,
    pub active_owners: u32,
    pub active_streams: u32,
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
