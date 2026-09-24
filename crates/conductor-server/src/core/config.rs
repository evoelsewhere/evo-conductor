use crate::core::constants::server::{
    DEFAULT_DATABASE_URL, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_WEB_DIST, ENV_DATABASE_URL, ENV_HOST,
    ENV_PORT, ENV_WEB_DIST,
};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RealtimeConfig {
    pub max_connections: usize,
    pub max_connections_per_secret: usize,
    pub max_concurrent_handshakes: usize,
    pub broadcast_capacity: usize,
    pub heartbeat_seconds: u64,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            max_connections: 10_000,
            max_connections_per_secret: 4,
            max_concurrent_handshakes: 256,
            broadcast_capacity: 512,
            heartbeat_seconds: 20,
        }
    }
}

impl RealtimeConfig {
    /// Applies limits saved in the database over the environment baseline.
    pub fn with_overrides(
        mut self,
        overrides: &conductor_storage::repos::NetworkOverrides,
    ) -> Self {
        if let Some(v) = overrides.realtime_max_connections {
            self.max_connections = v as usize;
        }
        if let Some(v) = overrides.realtime_max_per_secret {
            self.max_connections_per_secret = v as usize;
        }
        if let Some(v) = overrides.realtime_heartbeat_seconds {
            self.heartbeat_seconds = u64::from(v);
        }
        self
    }
}

/// Sync schedule for the models.dev price catalog. Conductor prices telemetry
/// from its own rate table, so this is what keeps that table current.
#[derive(Debug, Clone)]
pub struct ModelPricingConfig {
    pub enabled: bool,
    pub url: String,
    pub refresh_hours: u64,
}

impl Default for ModelPricingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            url: crate::core::model_pricing::MODELS_DEV_URL.to_string(),
            // Matches EvoFlux's own catalog TTL, so the two sides drift by at
            // most one refresh window.
            refresh_hours: 24,
        }
    }
}

/// SMTP settings for outbound transactional email (invite-to-connect links,
/// Jira usage-report notifications). Disabled by default — no email is sent
/// until an operator configures a real SMTP relay.
#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub enabled: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_address: String,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: String::new(),
            smtp_port: 587,
            smtp_username: String::new(),
            smtp_password: String::new(),
            from_address: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub web_dist: PathBuf,
    pub realtime: RealtimeConfig,
    pub model_pricing: ModelPricingConfig,
    pub email: EmailConfig,
}

impl Config {
    pub fn from_env() -> Self {
        let realtime_defaults = RealtimeConfig::default();
        let pricing_defaults = ModelPricingConfig::default();
        Self {
            database_url: std::env::var(ENV_DATABASE_URL)
                .unwrap_or_else(|_| DEFAULT_DATABASE_URL.into()),
            host: std::env::var(ENV_HOST).unwrap_or_else(|_| DEFAULT_HOST.into()),
            port: std::env::var(ENV_PORT)
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(DEFAULT_PORT),
            web_dist: std::env::var(ENV_WEB_DIST)
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_WEB_DIST)),
            realtime: RealtimeConfig {
                max_connections: env_usize(
                    "CONDUCTOR_REALTIME_MAX_CONNECTIONS",
                    realtime_defaults.max_connections,
                ),
                max_connections_per_secret: env_usize(
                    "CONDUCTOR_REALTIME_MAX_CONNECTIONS_PER_SECRET",
                    realtime_defaults.max_connections_per_secret,
                ),
                max_concurrent_handshakes: env_usize(
                    "CONDUCTOR_REALTIME_MAX_CONCURRENT_HANDSHAKES",
                    realtime_defaults.max_concurrent_handshakes,
                ),
                broadcast_capacity: env_usize(
                    "CONDUCTOR_REALTIME_BROADCAST_CAPACITY",
                    realtime_defaults.broadcast_capacity,
                ),
                heartbeat_seconds: env_u64(
                    "CONDUCTOR_REALTIME_HEARTBEAT_SECONDS",
                    realtime_defaults.heartbeat_seconds,
                )
                .clamp(5, 300),
            },
            model_pricing: ModelPricingConfig {
                enabled: env_bool("CONDUCTOR_MODEL_PRICING_ENABLED", pricing_defaults.enabled),
                url: std::env::var("CONDUCTOR_MODEL_PRICING_URL").unwrap_or(pricing_defaults.url),
                refresh_hours: env_u64(
                    "CONDUCTOR_MODEL_PRICING_REFRESH_HOURS",
                    pricing_defaults.refresh_hours,
                )
                .clamp(1, 24 * 30),
            },
            email: {
                let defaults = EmailConfig::default();
                EmailConfig {
                    enabled: env_bool("CONDUCTOR_EMAIL_ENABLED", defaults.enabled),
                    smtp_host: std::env::var("CONDUCTOR_SMTP_HOST").unwrap_or(defaults.smtp_host),
                    smtp_port: std::env::var("CONDUCTOR_SMTP_PORT")
                        .ok()
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(defaults.smtp_port),
                    smtp_username: std::env::var("CONDUCTOR_SMTP_USERNAME")
                        .unwrap_or(defaults.smtp_username),
                    smtp_password: std::env::var("CONDUCTOR_SMTP_PASSWORD")
                        .unwrap_or(defaults.smtp_password),
                    from_address: std::env::var("CONDUCTOR_EMAIL_FROM")
                        .unwrap_or(defaults.from_address),
                }
            },
        }
    }

    pub fn bind_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
        .max(1)
}

fn env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
        Err(_) => default,
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
