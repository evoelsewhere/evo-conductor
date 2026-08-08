use chrono::{Duration, Utc};
use conductor_domain::{ConductorError, PrimaryRole, Result};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Clone)]
pub struct JwtService {
    secret: String,
    ttl_hours: i64,
}

impl JwtService {
    pub fn new(secret: impl Into<String>, ttl_hours: i64) -> Self {
        Self {
            secret: secret.into(),
            ttl_hours,
        }
    }

    pub fn issue(&self, user_id: Uuid, email: &str, role: PrimaryRole) -> Result<(String, i64)> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.ttl_hours);
        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            role: role.as_str().to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| ConductorError::msg(format!("jwt encode failed: {e}")))?;

        Ok((token, exp.timestamp()))
    }

    pub fn verify(&self, token: &str) -> Result<Claims> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| ConductorError::Unauthorized)?;
        Ok(data.claims)
    }
}
