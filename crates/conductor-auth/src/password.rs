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
