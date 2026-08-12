use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::CollectionLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub id: Uuid,
    pub project_name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub bind_host: String,
    pub bind_port: u16,
    pub public_url: Option<String>,
    /// Optional project mark URL; when absent the UI falls back to the EvoFlux glyph.
    pub logo_url: Option<String>,
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
    pub display_name: Option<String>,
    pub logo_url: Option<String>,
    pub public_url: Option<String>,
    pub sso_enabled: bool,
}

/// Public project identity shared with every authenticated member (sidebar brand).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBranding {
    pub project_name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub project_name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub bind_host: String,
    pub bind_port: u16,
    pub public_url: Option<String>,
    pub logo_url: Option<String>,
    pub realtime: RealtimeSettings,
    pub data_policy: DataPolicySettings,
    pub sso: SsoConfig,
    pub storage: StorageSettings,
}

/// Project policy advertised to every registered EvoFlux installation.
/// L0 disables usage telemetry, L1 collects operational metadata, and L2
/// allows the richer privacy-safe resource attribution contract.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DataPolicySettings {
    pub collection_level: CollectionLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UpdateDataPolicyRequest {
    pub collection_level: CollectionLevel,
}

/// Project-scoped object storage selection. S3 and Azure credentials use their
/// process credential chains. A Git HTTPS token is accepted only as a
/// write-only update field and must never be serialized back or persisted in
/// SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StorageBackend {
    #[default]
    Local,
    S3,
    AzureBlob,
    Git,
}

impl StorageBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::S3 => "s3",
            Self::AzureBlob => "azure_blob",
            Self::Git => "git",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "s3" => Self::S3,
            "azure_blob" | "azure" => Self::AzureBlob,
            "git" => Self::Git,
            _ => Self::Local,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GitAuthMode {
    /// Use the Git process environment: SSH agent, workload-mounted key or an
    /// operator-configured credential helper.
    #[default]
    Environment,
    /// Use a write-only HTTPS access token stored outside the relational DB.
    HttpsToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalStorageSettings {
    /// Absolute path, or a path relative to CONDUCTOR_DATA_DIR.
    pub root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct S3StorageSettings {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub path_style: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AzureBlobStorageSettings {
    pub account: String,
    pub container: String,
    pub endpoint: Option<String>,
    #[serde(default)]
    pub prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStorageSettings {
    pub repository_url: String,
    #[serde(default = "default_git_branch")]
    pub branch: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub auth_mode: GitAuthMode,
    pub username: Option<String>,
    /// Write-only token/password. Deserialized from an update request but
    /// never serialized into API responses or `instance.storage_config`.
    #[serde(default, skip_serializing)]
    pub credential: Option<String>,
    /// Request-only command for deleting the credential file.
    #[serde(default, skip_serializing)]
    pub clear_credential: bool,
    /// Safe response/persistence metadata; never proves a credential is valid.
    #[serde(default)]
    pub credential_set: bool,
}

impl Default for GitStorageSettings {
    fn default() -> Self {
        Self {
            repository_url: String::new(),
            branch: default_git_branch(),
            prefix: String::new(),
            auth_mode: GitAuthMode::Environment,
            username: None,
            credential: None,
            clear_credential: false,
            credential_set: false,
        }
    }
}

fn default_git_branch() -> String {
    "main".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageSettings {
    pub backend: StorageBackend,
    #[serde(default)]
    pub local: LocalStorageSettings,
    #[serde(default)]
    pub s3: S3StorageSettings,
    #[serde(default)]
    pub azure_blob: AzureBlobStorageSettings,
    #[serde(default)]
    pub git: GitStorageSettings,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Local,
            local: LocalStorageSettings::default(),
            s3: S3StorageSettings::default(),
            azure_blob: AzureBlobStorageSettings::default(),
            git: GitStorageSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStorageRequest {
    pub storage: StorageSettings,
    /// Backend changes are rejected unless existing objects are migrated.
    #[serde(default = "default_true")]
    pub migrate_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMigrationResult {
    pub storage: StorageSettings,
    pub objects_copied: u64,
    pub bytes_copied: u64,
}

fn default_true() -> bool {
    true
}

/// Operator-tunable realtime (SSE) limits. Values unset in the database fall
/// back to the environment configuration the server was started with.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RealtimeSettings {
    pub max_connections: u32,
    pub max_connections_per_secret: u32,
    pub heartbeat_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInstanceRequest {
    pub project_name: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub public_url: Option<String>,
    pub logo_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNetworkRequest {
    pub bind_host: String,
    pub bind_port: u16,
    pub public_url: Option<String>,
    pub realtime: RealtimeSettings,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_credential_is_write_only_and_never_serialized() {
        let settings: StorageSettings = serde_json::from_value(serde_json::json!({
            "backend": "git",
            "git": {
                "repository_url": "https://git.example.test/acme/resources.git",
                "branch": "main",
                "prefix": "conductor",
                "auth_mode": "https_token",
                "username": "oauth2",
                "credential": "secret-token",
                "credential_set": false
            }
        }))
        .unwrap();

        assert_eq!(settings.backend, StorageBackend::Git);
        assert_eq!(settings.git.credential.as_deref(), Some("secret-token"));
        let serialized = serde_json::to_value(settings).unwrap();
        assert!(serialized["git"].get("credential").is_none());
        assert!(serialized["git"].get("clear_credential").is_none());
    }
}
