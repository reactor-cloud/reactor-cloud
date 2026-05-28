//! Reactor Connect — agent-shaped integration with third-party systems
//!
//! This crate implements the Connect capability per `docs/reactor-connect.design.md`:
//! - Connector discovery, configuration, and credential management
//! - Actions: typed RPC calls to third-party APIs
//! - Streams: bidirectional data replication (v0.2+)
//! - Webhooks: inbound event receivers with signature verification
//! - Sandbox mode for safe testing before production sync
//!
//! # Features
//!
//! - `runtime-native` (default) — First-party Rust connectors (Stripe, Slack, Linear, GitHub, Salesforce)
//! - `runtime-manifest` — Airbyte Low-Code CDK YAML interpreter (~100+ connectors)
//! - `runtime-airbyte` — Airbyte container runner via reactor-jobs (G3 only)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
#[cfg(feature = "runtime-native")]
pub mod connectors;
pub mod credentials;
pub mod descriptor;
pub mod error;
pub mod middleware;
pub mod protocol;
pub mod router;
pub mod routes;
pub mod runtime;
pub mod service;
pub mod sink;
pub mod sync;
pub mod state;
pub mod store;

// v0.2+ modules (stubbed for now)
pub mod action;
pub mod audit;
pub mod policy;
pub mod sandbox;
pub mod stream;
pub mod webhook;

use once_cell::sync::Lazy;
use regex::Regex;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Regex for valid connector instance names (lowercase alphanumeric with hyphens, 3-63 chars).
pub static INSTANCE_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9-]{1,61}[a-z0-9]$").expect("invalid instance name regex")
});

/// Regex for valid connection names (lowercase alphanumeric with hyphens, 3-63 chars).
pub static CONNECTION_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9-]{1,61}[a-z0-9]$").expect("invalid connection name regex")
});

/// Regex for valid action names (lowercase alphanumeric with underscores, 1-64 chars).
pub static ACTION_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-zA-Z0-9_]{0,63}$").expect("invalid action name regex")
});

// Re-exports
pub use config::ConnectConfig;
pub use descriptor::{
    ActionDescriptor, AuthDescriptor, AuthKind, ConnectorCapabilities, ConnectorDescriptor,
    DryRunSupport, IdempotencyHint, RateLimitDescriptor, SideEffectKind, StreamDescriptor,
    VerificationKind, WebhookDescriptor,
};
pub use error::ConnectError;
pub use protocol::{ConnectorMessage, ConnectionStatus};
pub use router::router;
pub use runtime::{ConnectorRuntime, RuntimeKind};
#[cfg(feature = "runtime-native")]
pub use runtime::NativeRuntime;
#[cfg(feature = "runtime-manifest")]
pub use runtime::ManifestRuntime;
pub use state::{ConnectCtx, ConnectState};
pub use store::{
    ActionInvocationRecord, ConnectStore, ConnectTx, Connection, ConnectionId, Instance,
    InstanceId, NewConnection, NewInstance, NewReceiver, PgConnectStore, Receiver, ReceiverId,
    StateBundle, SyncRunId, SyncRunRecord,
};

use utoipa::OpenApi;

/// OpenAPI documentation for the connect service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor Connect API",
        version = "1.0.0",
        description = "Agent-shaped integration with third-party systems via Actions, Streams, and Webhooks"
    ),
    paths(
        routes::health::health,
    ),
    components(schemas(
        routes::health::HealthResponse,
    )),
    tags(
        (name = "connect", description = "Connector management and invocation"),
        (name = "instances", description = "Connector instance configuration"),
        (name = "actions", description = "Action invocation"),
        (name = "webhooks", description = "Webhook receivers"),
        (name = "streams", description = "Data stream connections (v0.2+)"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

/// Returns the OpenAPI specification for the connect service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
