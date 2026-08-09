//! Shared foundations for the domain layer.
//!
//! `conductor-domain` is the crate every other crate already depends on and the
//! only one with no I/O, so a genuinely cross-crate fact belongs here. There is
//! deliberately no separate `shared` or `common` crate: a second dependency
//! that everything pulls in creates ambiguity about where a new value goes, and
//! that ambiguity is what turns such crates into dumping grounds.

pub mod constants;
pub mod error;
