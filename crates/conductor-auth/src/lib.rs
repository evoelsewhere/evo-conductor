//! Auth helpers: password hashing, JWT sessions, connection tokens, OIDC/MS SSO.

pub mod core;

mod jwt;
mod oidc;
mod password;
mod secret_token;

pub use jwt::{Claims, JwtService};
pub use oidc::{
    begin_authorization, default_scopes, exchange_code, normalize_issuer,
    validate_oidc_redirect_uri, validate_oidc_url, OidcAuthRequest, OidcProfile,
};
pub use password::{generate_temp_password, hash_password, verify_password};
pub use secret_token::{generate_connection_token, hash_token};
