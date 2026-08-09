//! Sizes of the random material behind each credential.

/// Bytes behind a connection token's lookup prefix, hex-encoded to eight chars.
pub const CONNECTION_TOKEN_PREFIX_BYTES: usize = 4;

/// Bytes behind a connection token's secret, base64url-encoded.
pub const CONNECTION_TOKEN_SECRET_BYTES: usize = 24;

/// Bytes behind the per-instance JWT signing secret, hex-encoded.
pub const JWT_SECRET_BYTES: usize = 32;
