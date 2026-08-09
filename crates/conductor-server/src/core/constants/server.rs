//! Startup configuration: environment variable names and their defaults.

pub const ENV_DATABASE_URL: &str = "CONDUCTOR_DATABASE_URL";
pub const ENV_HOST: &str = "CONDUCTOR_HOST";
pub const ENV_PORT: &str = "CONDUCTOR_PORT";
pub const ENV_WEB_DIST: &str = "CONDUCTOR_WEB_DIST";

pub const DEFAULT_DATABASE_URL: &str = "sqlite:data/conductor.db?mode=rwc";
pub const DEFAULT_HOST: &str = "0.0.0.0";
pub const DEFAULT_PORT: u16 = 4700;
pub const DEFAULT_WEB_DIST: &str = "apps/web/dist";
