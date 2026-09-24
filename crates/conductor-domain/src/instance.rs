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
    pub email: EmailSettings,
    pub jira: JiraSettings,
    /// Human-readable configuration gaps for this project, computed fresh on
    /// every read (never persisted) — e.g. a missing public URL blocks
    /// invite-to-connect emails. Empty when nothing needs attention.
    pub warnings: Vec<String>,
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

/// Project-scoped object storage selection. Azure credentials use the process
/// credential chain. S3 accepts explicit `access_key_id`/`secret_access_key`
/// (falling back to the process AWS credential chain, e.g. an IAM role, when
/// left blank) so a non-EC2 deployment doesn't have to wait out an IMDS
/// timeout to reach a self-hosted/MinIO-style endpoint. A Git HTTPS token is
/// accepted only as a write-only update field and must never be serialized
/// back or persisted in SQL — the S3 secret key follows the same pattern.
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
    /// Optional explicit access key ID. Blank defers to the process AWS
    /// credential chain (env vars, IAM role, IMDS).
    #[serde(default)]
    pub access_key_id: String,
    /// Write-only secret access key. Deserialized from an update request but
    /// never serialized into API responses or `instance.storage_config`.
    #[serde(default, skip_serializing)]
    pub secret_access_key: Option<String>,
    /// Request-only command for deleting the stored secret key.
    #[serde(default, skip_serializing)]
    pub clear_secret_access_key: bool,
    /// Safe response/persistence metadata; never proves a key is valid.
    #[serde(default)]
    pub secret_access_key_set: bool,
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

/// Outbound SMTP configuration for transactional email (invite-to-connect
/// links, Jira usage-report notifications). Stored per-project so an operator
/// can configure it from the Settings UI instead of process environment
/// variables; the environment (`CONDUCTOR_SMTP_*`) remains a fallback used
/// only while this is disabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_username: String,
    /// Write-only password. Deserialized from an update request but never
    /// serialized into API responses or `instance.email_config`.
    #[serde(default, skip_serializing)]
    pub smtp_password: Option<String>,
    /// Request-only command for deleting the stored password.
    #[serde(default, skip_serializing)]
    pub clear_smtp_password: bool,
    /// Safe response/persistence metadata; never proves a password is valid.
    #[serde(default)]
    pub smtp_password_set: bool,
    #[serde(default)]
    pub from_address: String,
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: String::new(),
            smtp_port: default_smtp_port(),
            smtp_username: String::new(),
            smtp_password: None,
            clear_smtp_password: false,
            smtp_password_set: false,
            from_address: String::new(),
        }
    }
}

fn default_smtp_port() -> u16 {
    587
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEmailRequest {
    pub email: EmailSettings,
}

/// Jira connection for this project's usage reporting (Phase 4). Configured
/// from the Settings UI, never hardcoded — the token is an Atlassian API
/// token (Basic Auth with `email`), not an OAuth app; a 3-legged OAuth flow
/// needs a registered Atlassian app this deployment doesn't have, and a
/// token is what an admin can obtain today without one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiraSettings {
    #[serde(default)]
    pub enabled: bool,
    /// e.g. `https://your-domain.atlassian.net`.
    #[serde(default)]
    pub site_url: String,
    /// The Atlassian account email the API token belongs to.
    #[serde(default)]
    pub email: String,
    /// Write-only API token. Deserialized from an update request but never
    /// serialized into API responses or `instance.jira_config`.
    #[serde(default, skip_serializing)]
    pub api_token: Option<String>,
    /// Request-only command for deleting the stored token.
    #[serde(default, skip_serializing)]
    pub clear_api_token: bool,
    /// Safe response/persistence metadata; never proves a token is valid.
    #[serde(default)]
    pub api_token_set: bool,
    /// Issue key prefix usage reports are posted under by default, e.g. `SCRUM`.
    #[serde(default)]
    pub default_project_key: String,
    /// The specific issue usage-report comments are posted to, e.g. `SCRUM-1`.
    /// Comments attach to one issue, not a project — there is no
    /// project-level comment in Jira's model.
    #[serde(default)]
    pub report_issue_key: String,
    /// The last period (`YYYY-MM-DD`, the period's start date) a usage
    /// report was successfully posted for — read-only, set by the report
    /// action itself. Prevents the background loop (T4.5) from posting the
    /// same period twice; never blocks a manual "post now".
    #[serde(default)]
    pub last_reported_period: Option<String>,
    /// How often the background loop (T4.5) posts an automatic report, and
    /// the length of the period each one covers. Admin-configurable here
    /// rather than an environment variable or a hardcoded cadence — a
    /// project's reporting rhythm is exactly the kind of thing that
    /// shouldn't require a restart to change.
    #[serde(default = "default_report_interval_hours")]
    pub report_interval_hours: u32,
    /// Ordered `{pattern -> type_label}` rules an admin defines to resolve a
    /// synced task's display type (e.g. `create`/`review`/`fix`/`update`)
    /// from its title, first match wins. Falls back to the task's raw Jira
    /// `issuetype` when nothing matches — this never replaces that field,
    /// only overrides how it's labeled in the report.
    #[serde(default)]
    pub task_type_rules: Vec<TaskTypeRule>,
    /// Ordered `{pattern -> project_label}` rules resolving a synced task's
    /// sub-project/module (e.g. a `[PST]` prefix in the title) — for a Jira
    /// project whose issues actually track more than one product. Unlike
    /// `task_type_rules`, there is no Jira field to fall back to when
    /// nothing matches: the task simply carries no sub-project label.
    #[serde(default)]
    pub project_prefix_rules: Vec<ProjectPrefixRule>,
}

fn default_report_interval_hours() -> u32 {
    168
}

impl Default for JiraSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            site_url: String::new(),
            email: String::new(),
            api_token: None,
            clear_api_token: false,
            api_token_set: false,
            default_project_key: String::new(),
            report_issue_key: String::new(),
            report_interval_hours: default_report_interval_hours(),
            last_reported_period: None,
            task_type_rules: Vec::new(),
            project_prefix_rules: Vec::new(),
        }
    }
}

/// One `task_type_rules` entry: `pattern` is matched case-insensitively as a
/// substring against a synced task's title (e.g. `[Fix]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskTypeRule {
    pub pattern: String,
    pub type_label: String,
}

/// Resolves a task's display type: the first `task_type_rules` entry whose
/// pattern matches `title` (case-insensitive substring), else `issue_type`
/// unchanged.
pub fn resolve_task_type<'a>(
    rules: &'a [TaskTypeRule],
    title: &str,
    issue_type: &'a str,
) -> &'a str {
    let title_lower = title.to_lowercase();
    for rule in rules {
        if !rule.pattern.is_empty() && title_lower.contains(&rule.pattern.to_lowercase()) {
            return &rule.type_label;
        }
    }
    issue_type
}

/// One `project_prefix_rules` entry: `pattern` is matched case-insensitively
/// as a substring against a synced task's title (e.g. `[PST]`), for a Jira
/// project whose issues actually span more than one product/module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPrefixRule {
    pub pattern: String,
    pub project_label: String,
}

/// Resolves a task's sub-project label from the first matching
/// `project_prefix_rules` entry, or `None` when nothing matches — there is
/// no Jira field to fall back to here, unlike `resolve_task_type`.
pub fn resolve_project_label<'a>(rules: &'a [ProjectPrefixRule], title: &str) -> Option<&'a str> {
    let title_lower = title.to_lowercase();
    rules
        .iter()
        .find(|rule| !rule.pattern.is_empty() && title_lower.contains(&rule.pattern.to_lowercase()))
        .map(|rule| rule.project_label.as_str())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJiraRequest {
    pub jira: JiraSettings,
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
