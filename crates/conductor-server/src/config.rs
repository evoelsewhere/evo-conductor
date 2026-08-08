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
            database_url: std::env::var("CONDUCTOR_DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data/conductor.db?mode=rwc".into()),
            host: std::env::var("CONDUCTOR_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("CONDUCTOR_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(4700),
            web_dist: std::env::var("CONDUCTOR_WEB_DIST")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("apps/web/dist")),
        }
    }

    pub fn bind_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }
}
