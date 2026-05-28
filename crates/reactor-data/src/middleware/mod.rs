//! Middleware for reactor-data.
//!
//! - Authentication: Bearer token extraction and validation
//! - Request context: DataCtx construction and insertion

mod auth;

pub use auth::{auth_middleware, DataCtx};
