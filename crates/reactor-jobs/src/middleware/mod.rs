//! Middleware for authentication and authorization.

pub mod auth;

pub use auth::require_auth;
