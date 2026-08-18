mod auth_user;
mod connection_secret;

pub use auth_user::{authenticate_browser_user, AuthUser};
pub(crate) use connection_secret::{
    authenticate_connection_principal, connection_principal_scope,
    mark_connection_secret_used_if_due,
};
pub use connection_secret::{authenticate_connection_secret, ConnectionPrincipal};
