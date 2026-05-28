//! Boot sequence modules.
//!
//! These modules handle the server startup sequence:
//! - Building shared resources (pool, cache, http client, vault)
//! - Initializing tracing
//! - Setting up shutdown coordination
//! - Running migrations across capabilities
//! - Building the auth bundle for inter-capability auth
//! - Tenant context injection for multi-tenancy support
//! - Vault initialization for secrets management
//! - Proxy trust validation for X-Forwarded-* headers

#[cfg(feature = "cap-auth")]
pub mod auth;
#[cfg(feature = "cap-cloud")]
pub mod cloud;
pub mod migrate;
pub mod pool;
pub mod proxy;
pub mod shutdown;
pub mod tenant;
pub mod tracing;
pub mod vault;

#[cfg(feature = "cap-auth")]
pub use auth::AuthBundle;
#[cfg(feature = "cap-cloud")]
pub use cloud::{bootstrap as cloud_bootstrap, CloudBootstrapConfig, CloudBootstrapError};
pub use migrate::run_all as run_migrations;
pub use pool::SharedResources;
pub use proxy::{trusted_proxy_middleware, RealClientIp, TrustedProxies};
pub use shutdown::{shutdown_signal, ShutdownHandle};
pub use tenant::{Tenant, TenantProvider, TenantResolutionError};
pub use vault::{build_vault, CachedVault};
