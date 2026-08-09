//! The shape of an EvoFlux connection token.
//!
//! `conductor-auth` writes this when generating a token and `conductor-server`
//! checks it when validating one. Two crates, two independent literals until
//! these constants existed: changing one silently made the other reject every
//! token.

/// Prefix identifying an EvoFlux connection token: `evc_<prefix>_<secret>`.
pub const CONNECTION_TOKEN_PREFIX: &str = "evc_";

/// Separator between the lookup prefix and the secret.
pub const CONNECTION_TOKEN_SEPARATOR: &str = "_";
