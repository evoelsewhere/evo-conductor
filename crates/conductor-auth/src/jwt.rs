use chrono::{Duration, Utc};
use conductor_domain::{ConductorError, PrimaryRole, Result};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub ver: i64,
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
}

const JWT_ISSUER: &str = "evo-conductor";
const JWT_AUDIENCE: &str = "evo-conductor-web";

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

    pub fn issue(
        &self,
        user_id: Uuid,
        email: &str,
        role: PrimaryRole,
        session_version: i64,
    ) -> Result<(String, i64)> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.ttl_hours);
        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            role: role.as_str().to_string(),
            ver: session_version,
            iss: JWT_ISSUER.to_string(),
            aud: JWT_AUDIENCE.to_string(),
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
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[JWT_ISSUER]);
        validation.set_audience(&[JWT_AUDIENCE]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = 30;
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .map_err(|_| ConductorError::Unauthorized)?;
        Ok(data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_session_version() {
        let service = JwtService::new("a-test-secret-with-enough-entropy", 1);
        let user_id = Uuid::new_v4();
        let (token, _) = service
            .issue(user_id, "admin@example.test", PrimaryRole::Admin, 7)
            .expect("token should be issued");
        let claims = service.verify(&token).expect("token should verify");
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.ver, 7);
        assert_eq!(claims.iss, JWT_ISSUER);
        assert_eq!(claims.aud, JWT_AUDIENCE);
    }

    #[test]
    fn rejects_token_from_another_instance_secret() {
        let issuer = JwtService::new("first-test-secret", 1);
        let verifier = JwtService::new("second-test-secret", 1);
        let (token, _) = issuer
            .issue(Uuid::new_v4(), "user@example.test", PrimaryRole::User, 0)
            .expect("token should be issued");
        assert!(verifier.verify(&token).is_err());
    }
}
