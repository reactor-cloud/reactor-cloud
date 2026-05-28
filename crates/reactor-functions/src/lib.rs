//! Reactor Functions capability — sandboxed HTTP handlers with wasm/bun/lambda runtimes
//!
//! This crate implements the Functions capability per `docs/reactor-functions.design.md`:
//! - A `FunctionRuntime` trait abstracting wasm/bun/lambda runtimes
//! - Bundle storage in `reactor-storage`'s `_reactor_functions` system bucket
//! - Invoke-time policy enforcement via `reactor-policy`
//! - Streaming request/response bodies
//! - Per-function env/secrets, concurrency caps, and timeouts

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod bundle;
pub mod config;
pub mod error;
pub mod middleware;
pub mod policy;
pub mod router;
pub mod routes;
pub mod runtime;
pub mod state;
pub mod store;

use once_cell::sync::Lazy;
use regex::Regex;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Regex for valid function names (lowercase alphanumeric with hyphens, 3-63 chars).
pub static FUNCTION_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9-]{1,61}[a-z0-9]$").expect("invalid function name regex")
});

/// Regex for valid env key names (uppercase alphanumeric with underscores).
pub static ENV_KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Z][A-Z0-9_]{0,127}$").expect("invalid env key regex")
});

// Re-exports
pub use bundle::{
    JobBackoffStrategy, JobConfig, JobRetryConfig, JobTriggerConfig, JobTriggerKind,
    Manifest, RuntimeKind,
};
pub use config::{Deployment, FunctionsConfig};
pub use error::FunctionsError;
pub use router::router;
pub use routes::HealthResponse;
pub use runtime::{
    DeploymentHandle, FunctionRuntime, IncomingRequest, InvokeResult, Limits, OutgoingResponse,
    RuntimeRegistry,
};
pub use state::{FunctionCtx, FunctionsState};
pub use store::{
    AuditEvent, AuditEventCreate, AuditEventId, Deployment as DeploymentRecord, DeploymentCreate,
    DeploymentId, DeploymentStatus, EnvVar, Function, FunctionCreate, FunctionId, FunctionsStore,
    FunctionsTx, Invocation, InvocationCreate, InvocationId, PgFunctionsStore, Policy, PolicyId,
};

#[cfg(feature = "runtime-bun")]
pub use runtime::{BunRuntime, BunRuntimeConfig};
#[cfg(feature = "runtime-lambda")]
pub use runtime::{LambdaRuntime, LambdaRuntimeConfig};
#[cfg(feature = "runtime-wasm")]
pub use runtime::{WasmRuntime, WasmRuntimeConfig};

use utoipa::OpenApi;

/// OpenAPI documentation for the functions service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor Functions API",
        version = "1.0.0",
        description = "Serverless functions with wasm/bun/lambda runtimes"
    ),
    paths(
        routes::health::health,
    ),
    components(schemas(
        routes::HealthResponse,
    )),
    tags(
        (name = "functions", description = "Function management and invocation"),
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

/// Returns the OpenAPI specification for the functions service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
