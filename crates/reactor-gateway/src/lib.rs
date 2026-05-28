//! Edge gateway routing and Caddy integration for Reactor.
//!
//! This crate provides:
//! - Routing table management for tenant requests
//! - Caddy admin API client and JSON config builder
//! - On-demand TLS endpoint for custom domains
//! - Sync loop for applying routing changes to Caddy

pub mod caddy_admin;
pub mod error;
pub mod on_demand_tls;
pub mod routing;
pub mod snapshot;
pub mod sync;

#[cfg(feature = "postgres")]
pub mod store;

pub use caddy_admin::CaddyAdminClient;
pub use error::{GatewayError, GatewayResult};
pub use routing::{BackendKind, BackendTarget, Route, RoutingTable, TlsMode};
pub use snapshot::ConfigSnapshot;
pub use sync::SyncLoop;
