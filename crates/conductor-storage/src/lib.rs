//! Infrastructure — multi-backend persistence (SQLite / Postgres / MySQL).

mod db;
mod dialect;
mod mapping;
mod migrate;
pub mod repos;

pub use db::Db;
pub use dialect::DatabaseKind;
