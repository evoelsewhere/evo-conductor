//! Facts that more than one crate depends on, one file per subject.
//!
//! A value belongs here only when two crates would otherwise each write their
//! own copy. Anything used inside a single crate lives in that crate's own
//! `core::constants`.

pub mod auth;
pub mod token;
