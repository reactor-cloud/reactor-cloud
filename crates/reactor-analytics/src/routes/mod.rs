//! HTTP route handlers.

pub mod admin;
pub mod consent;
pub mod erasure;
pub mod health;
pub mod ingest;
pub mod metrics;
pub mod query;
pub mod snippet;

pub use health::health;
