//! Bootstrap utilities for tenant provisioning.
//!
//! - [`SchemaBootstrap`]: Create tenant schemas and run capability migrations
//! - [`VaultBootstrap`]: Initialize vault transit keys and KV secrets

mod schema;
mod vault;

pub use schema::{BootstrapConfig, SchemaBootstrap};
pub use vault::VaultBootstrap;
