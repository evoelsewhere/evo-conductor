use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub id: Uuid,
    pub project_name: String,
    pub display_name: Option<String>,
    pub bind_host: String,
    pub bind_port: u16,
    pub public_url: Option<String>,
    pub setup_completed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoConfig {
    pub enabled: bool,
    pub provider: SsoProvider,
    pub issuer_url: Option<String>,
    pub client_id: Option<String>,
    /// Never returned to clients after create — only presence flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret_set: Option<bool>,
    pub redirect_uri: Option<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SsoProvider {
    #[default]
    Oidc,
    /// Placeholder for GitHub Copilot / Codex-style device or OAuth flows.
    Github,
    AzureAd,
    Google,
    Custom,
}

impl SsoProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Oidc => "oidc",
            Self::Github => "github",
            Self::AzureAd => "azure_ad",
            Self::Google => "google",
            Self::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "github" => Self::Github,
            "azure_ad" | "azuread" => Self::AzureAd,
            "google" => Self::Google,
            "custom" => Self::Custom,
            _ => Self::Oidc,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupRequest {
    pub project_name: String,
    pub display_name: Option<String>,
    pub bind_host: String,
    pub bind_port: u16,
    pub public_url: Option<String>,
    pub admin_email: String,
    pub admin_display_name: String,
    pub admin_password: String,
    pub sso: Option<SetupSsoRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupSsoRequest {
    pub enabled: bool,
    pub provider: SsoProvider,
    pub issuer_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
    pub scopes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatus {
    pub configured: bool,
    pub project_name: Option<String>,
    pub public_url: Option<String>,
    pub sso_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub project_name: String,
    pub display_name: Option<String>,
    pub bind_host: String,
    pub bind_port: u16,
    pub public_url: Option<String>,
    pub sso: SsoConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInstanceRequest {
    pub project_name: Option<String>,
    pub display_name: Option<String>,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSsoRequest {
    pub enabled: bool,
    pub provider: SsoProvider,
    pub issuer_url: Option<String>,
    pub client_id: Option<String>,
    /// Omit or null to keep the existing secret.
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
    pub scopes: Option<Vec<String>>,
}
