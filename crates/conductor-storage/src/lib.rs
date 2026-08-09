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
pub use db::Db;
