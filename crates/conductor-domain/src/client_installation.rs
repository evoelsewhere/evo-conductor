use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{PrimaryRole, SubRole, Tag};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientPlatform {
    Macos,
    Linux,
    Windows,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterClientRequest {
    pub installation_key: Uuid,
    pub display_name: String,
    pub platform: ClientPlatform,
    pub evoflux_version: String,
    pub workspace_association: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInstallation {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub user_id: Uuid,
    pub installation_key: Uuid,
    pub display_name: String,
    pub platform: ClientPlatform,
    pub evoflux_version: String,
    pub workspace_association: Option<String>,
    pub connected_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInstallationSummary {
    pub id: Uuid,
    pub display_name: String,
    pub platform: ClientPlatform,
    pub evoflux_version: String,
    pub connected_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

impl From<ClientInstallation> for ClientInstallationSummary {
    fn from(value: ClientInstallation) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
            platform: value.platform,
            evoflux_version: value.evoflux_version,
            connected_at: value.connected_at,
            last_seen_at: value.last_seen_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredInstallation {
    pub id: Uuid,
    pub display_name: String,
    pub heartbeat_interval_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientProject {
    pub id: Uuid,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMember {
    pub id: Uuid,
    pub display_name: String,
    pub primary_role: PrimaryRole,
    pub sub_roles: Vec<SubRole>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionLevel {
    L0,
    L1,
    L2,
}

impl CollectionLevel {
    pub fn parse(value: &str) -> Self {
        match value {
            "L0" => Self::L0,
            "L2" => Self::L2,
            _ => Self::L1,
        }
    }

    pub fn telemetry_enabled(self) -> bool {
        !matches!(self, Self::L0)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::L0 => "L0",
            Self::L1 => "L1",
            Self::L2 => "L2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientTelemetryPolicy {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPolicy {
    pub collection_level: CollectionLevel,
    pub telemetry: ClientTelemetryPolicy,
    pub privacy_notice_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterClientResponse {
    pub installation: RegisteredInstallation,
    pub project: ClientProject,
    pub member: ClientMember,
    pub policy: ClientPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientHeartbeatRequest {
    pub installation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHeartbeatResponse {
    pub server_time: DateTime<Utc>,
    pub heartbeat_interval_seconds: u32,
    pub connection_state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_level_defaults_unknown_values_to_l1() {
        assert_eq!(CollectionLevel::parse("unknown"), CollectionLevel::L1);
        assert!(CollectionLevel::L1.telemetry_enabled());
        assert!(!CollectionLevel::L0.telemetry_enabled());
    }
}
