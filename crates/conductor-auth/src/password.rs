use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use conductor_domain::{ConductorError, Result};
use rand::rngs::OsRng;

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ConductorError::msg(format!("password hash failed: {e}")))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(password_hash)
        .map_err(|e| ConductorError::msg(format!("invalid password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Hash a password without monopolizing an async runtime worker.
///
/// Argon2 is deliberately CPU and memory intensive. HTTP handlers must use
/// this wrapper so a burst of member invites cannot starve database I/O,
/// telemetry ingestion, heartbeats or realtime delivery.
pub async fn hash_password_async(password: String) -> Result<String> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| ConductorError::Internal)?
}

/// Verify a password on Tokio's blocking pool for the same reason as
/// [`hash_password_async`].
pub async fn verify_password_async(password: String, password_hash: String) -> Result<bool> {
    tokio::task::spawn_blocking(move || verify_password(&password, &password_hash))
        .await
        .map_err(|_| ConductorError::Internal)?
}

/// Human-friendly temp password for invites (never logged).
pub fn generate_temp_password() -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    let mut chars: Vec<u8> = (0..12)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())])
        .collect();
    // Insert a dash for readability: XXXX-XXXX-XXXX
    chars.insert(4, b'-');
    chars.insert(9, b'-');
    String::from_utf8(chars).unwrap_or_else(|_| "TempPass-9xK2".into())
}

#[cfg(test)]
mod tests {
    use super::{hash_password_async, verify_password_async};

    #[tokio::test]
    async fn async_password_round_trip_runs_on_blocking_pool() {
        let password = "a-local-password-for-testing".to_string();
        let hash = hash_password_async(password.clone()).await.unwrap();
        assert!(verify_password_async(password, hash).await.unwrap());
    }
}
