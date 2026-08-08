//! Auth helpers: password hashing, JWT sessions, connection tokens, OIDC/MS SSO.

mod jwt;
mod oidc;
mod password;
mod secret_token;

pub use jwt::{Claims, JwtService};
pub use oidc::{
    begin_authorization, default_scopes, exchange_code, normalize_issuer, OidcAuthRequest,
    OidcProfile,
};
pub use password::{hash_password, verify_password};
pub use secret_token::{generate_connection_token, hash_token};
