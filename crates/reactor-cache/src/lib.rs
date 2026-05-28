//! Reactor Cache — queue + KV + leader election abstraction
//!
//! This crate provides trait-based abstractions for:
//! - Queue operations (using `FOR UPDATE SKIP LOCKED` in Postgres)
//! - Key-value operations
//! - Leader election (using PostgreSQL advisory locks)
//!
//! v0 ships with `PostgresBackend`; Redis backend planned for v0.2.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod backend;
mod error;
mod kv;
pub mod leader;
mod postgres;
mod queue;

pub use backend::CacheBackend;
pub use error::CacheError;
pub use kv::KvOperations;
pub use leader::{pg_advisory_leader, PgAdvisoryLeader};
pub use postgres::PostgresBackend;
pub use queue::{QueueItem, QueueOperations};

// Re-export core leader types for convenience
pub use reactor_core::primitives::leader::{always_leader, AlwaysLeader, LeaderElect, LeaderGuard};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
