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

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub web_dist: PathBuf,
    pub realtime: RealtimeConfig,
}

impl Config {
    pub fn from_env() -> Self {
        let realtime_defaults = RealtimeConfig::default();
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

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
