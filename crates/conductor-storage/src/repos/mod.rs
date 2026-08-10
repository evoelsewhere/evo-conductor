pub mod client_installation;
pub mod dashboard;
pub mod instance;
pub mod resource;
pub mod role;
pub mod secret;
pub mod user;

pub use client_installation::{ClientInstallationRepo, RegisterInstallationError};
pub use dashboard::DashboardRepo;
pub use instance::{InstanceRepo, NetworkOverrides, SsoConfigUpdate, SsoRuntime};
pub use resource::ResourceRepo;
pub use role::RoleRepo;
pub use secret::SecretRepo;
pub use user::{SsoLoginError, UserRepo};
