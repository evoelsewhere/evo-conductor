use serde::{Deserialize, Serialize};

use crate::ResourceCounts;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub project_name: String,
    pub members_total: u32,
    pub members_online: u32,
    pub secrets_active: u32,
    pub resources: ResourceCounts,
    pub sso_enabled: bool,
}
