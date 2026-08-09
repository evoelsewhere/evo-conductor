//! Values governing an in-flight OIDC exchange.

/// How long a pending exchange is remembered before it is purged.
pub const PENDING_TTL_SECS: u64 = 600;
