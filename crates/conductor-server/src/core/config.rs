use crate::core::constants::server::{
    DEFAULT_DATABASE_URL, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_WEB_DIST, ENV_DATABASE_URL, ENV_HOST,
    ENV_PORT, ENV_WEB_DIST,
};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub web_dist: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
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
        }
    }

    pub fn bind_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }
}
