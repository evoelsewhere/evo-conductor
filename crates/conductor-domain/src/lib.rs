//! Domain layer — entities, value objects, and domain errors.
//! No I/O, no framework dependencies.

pub mod error;
pub mod instance;
pub mod resource;
pub mod role;
pub mod secret;
pub mod telemetry;
pub mod user;

pub use error::{ConductorError, Result};
pub use instance::*;
pub use resource::*;
pub use role::{CreateSubRoleRequest, PrimaryRole, SubRole};
pub use secret::*;
pub use telemetry::*;
pub use user::*;
