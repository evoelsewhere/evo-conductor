//! Shared foundations for the storage layer: what a connection string looks
//! like, what the schema contains, and how rows become domain types.

pub mod constants;
pub mod dialect;
pub mod error;
pub mod mapping;
pub mod url;

pub use error::{
    InvalidPersistedCredential, InvalidPersistedPrincipal, InvalidPersistedResource,
    PersistedCredentialField, PersistedPrincipalField, PersistedResourceField,
    PersistedSecurityReason, StorageError, StorageResult,
};
