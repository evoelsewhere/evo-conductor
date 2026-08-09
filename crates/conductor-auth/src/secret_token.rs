use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use conductor_domain::core::constants::token::{
    CONNECTION_TOKEN_PREFIX, CONNECTION_TOKEN_SEPARATOR,
};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::core::constants::token::{CONNECTION_TOKEN_PREFIX_BYTES, CONNECTION_TOKEN_SECRET_BYTES};

/// Returns `(raw_token, prefix, sha256_hex_hash)`.
/// Raw token format: `evc_<prefix>_<random>` — shown once to the user.
pub fn generate_connection_token() -> (String, String, String) {
    let mut prefix_bytes = [0u8; CONNECTION_TOKEN_PREFIX_BYTES];
    let mut secret_bytes = [0u8; CONNECTION_TOKEN_SECRET_BYTES];
    rand::thread_rng().fill_bytes(&mut prefix_bytes);
    rand::thread_rng().fill_bytes(&mut secret_bytes);

    let prefix = hex::encode(prefix_bytes);
    let secret = URL_SAFE_NO_PAD.encode(secret_bytes);
    let raw = format!("{CONNECTION_TOKEN_PREFIX}{prefix}{CONNECTION_TOKEN_SEPARATOR}{secret}");
    let hash = hash_token(&raw);
    (raw, prefix, hash)
}

pub fn hash_token(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    hex::encode(digest)
}
