use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    pub const ALL: [Self; 3] = [
        Self::SubscribeResources,
        Self::ReportTelemetry,
        Self::SyncInventory,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SubscribeResources => "subscribe_resources",
            Self::ReportTelemetry => "report_telemetry",
            Self::SyncInventory => "sync_inventory",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "subscribe_resources" => Some(Self::SubscribeResources),
            "report_telemetry" => Some(Self::ReportTelemetry),
            "sync_inventory" => Some(Self::SyncInventory),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseSecretScopeError;

impl fmt::Display for ParseSecretScopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unknown secret scope")
    }
}

impl std::error::Error for ParseSecretScopeError {}

impl FromStr for SecretScope {
    type Err = ParseSecretScopeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(ParseSecretScopeError)
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
