//! Infrastructure primitive traits.
//!
//! These traits define the contracts for cross-cutting infrastructure concerns
//! that capabilities depend on. Implementations live in separate crates
//! (e.g., `reactor-vault`, `reactor-cache`) or as adapters within capabilities.
//!
//! # Traits
//!
//! - [`PubSub`] — Publish/subscribe messaging for realtime fanout
//! - [`LeaderElect`] — Leader election for background tasks in clusters
//! - [`Vault`] — Secret storage and transit encryption
//!
//! # Design principles
//!
//! 1. **Traits live here, impls live elsewhere.** This keeps `reactor-core`
//!    dependency-light and allows capabilities to remain database-agnostic.
//!
//! 2. **Default/simple impls where safe.** `AlwaysLeader` and `InProcessPubSub`
//!    are included here because they're zero-dependency and useful for single-node
//!    deployments.
//!
//! 3. **Tenant-scoped where applicable.** Vault operations take a `ProjectId`
//!    so they work identically in single-tenant and multi-tenant modes.

pub mod leader;
#[cfg(feature = "nats")]
pub mod nats_pubsub;
pub mod pubsub;
pub mod vault;

pub use leader::{AlwaysLeader, LeaderElect, LeaderGuard};
#[cfg(feature = "nats")]
pub use nats_pubsub::{nats_pubsub, NatsPubSub, NatsPubSubConfig};
pub use pubsub::{in_process_pubsub, InProcessPubSub, PubSub, PubSubError, Subscription};
pub use vault::{Ciphertext, SecretMetadata, SecretValue, Vault, VaultError};
