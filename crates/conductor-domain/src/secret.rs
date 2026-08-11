use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSecret {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub owner_user_id: Uuid,
    pub scopes: Vec<SecretScope>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretScope {
    /// Pull shared Agents, Skills and Plugin catalogs.
    SubscribeResources,
    /// Push local usage / performance telemetry.
    ReportTelemetry,
    /// Bidirectional sync of member workspace inventory.
    SyncInventory,
}

impl SecretScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SubscribeResources => "subscribe_resources",
            Self::ReportTelemetry => "report_telemetry",
            Self::SyncInventory => "sync_inventory",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSecretRequest {
    pub name: String,
    pub scopes: Vec<SecretScope>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedSecret {
    pub secret: ConnectionSecret,
    /// Raw token — shown once.
    pub token: String,
}
