//! Middleware for sites routes.

pub mod auth;
pub mod host_resolver;

pub use auth::auth_middleware;
pub use host_resolver::HostResolver;
