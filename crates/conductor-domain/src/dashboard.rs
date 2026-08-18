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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardHostMetricsScope {
    ConductorHost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardHostMetrics {
    pub scope: DashboardHostMetricsScope,
    pub sampled_at: DateTime<Utc>,
    pub cpu_usage_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub gpu_usage_percent: Option<f64>,
    pub vram_used_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
}

impl DashboardHostMetrics {
    pub fn unavailable(sampled_at: DateTime<Utc>) -> Self {
        Self {
            scope: DashboardHostMetricsScope::ConductorHost,
            sampled_at,
            cpu_usage_percent: None,
            memory_used_bytes: None,
            memory_total_bytes: None,
            gpu_usage_percent: None,
            vram_used_bytes: None,
            vram_total_bytes: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardFeedbackScope {
    Project,
    OwnedResources,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DashboardFeedbackDistribution {
    pub rating_1: u32,
    pub rating_2: u32,
    pub rating_3: u32,
    pub rating_4: u32,
    pub rating_5: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardFeedbackSummary {
    pub scope: DashboardFeedbackScope,
    pub count: u32,
    pub average_rating: Option<f64>,
    /// Ratings 4 and 5 are positive.
    pub positive_count: u32,
    pub positive_percent: Option<f64>,
    pub distribution: DashboardFeedbackDistribution,
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
