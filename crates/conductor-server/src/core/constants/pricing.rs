//! Model-pricing catalog fetch/cache settings.
//!
//! Mirrors the evoflux client's own catalog client
//! (`app/agent/providers/model_registry.py`): same source URL, same 24h
//! cache TTL, same short fetch timeout, so a stale cache degrades to the
//! Python client's behavior rather than a worse one.

pub const MODELS_DEV_URL: &str = "https://models.dev/api.json";
pub const MODELS_DEV_CACHE_PATH: &str = "data/cache/models-dev.json";
pub const MODELS_DEV_CACHE_TTL_SECONDS: u64 = 24 * 60 * 60;
pub const MODELS_DEV_FETCH_TIMEOUT_SECONDS: u64 = 5;
