//! Infrastructure — multi-backend persistence (SQLite / Postgres / MySQL).
//!
//! ```text
//! core/     connection strings, dialects, schema constants, row mapping
//! repos/    one repository per aggregate
//! db        the pool handle and repository accessors
//! migrate   schema creation
//! ```

pub mod core;
pub mod repos;

mod db;
mod migrate;

pub use core::dialect::DatabaseKind;
pub use core::error::{
    InvalidPersistedCredential, InvalidPersistedPrincipal, InvalidPersistedResource,
    PersistedCredentialField, PersistedPrincipalField, PersistedResourceField,
    PersistedSecurityReason, StorageError, StorageResult,
};
pub use db::Db;
pub use repos::{
    ApproveMemberAccess, ChangeMemberStatus, CredentialPolicyEffect, MemberAccessChange,
    MemberAccessError, MemberAccessResult, MemberSecuritySnapshot, MemberStatusReason,
    UpdateAccessProfile,
};
