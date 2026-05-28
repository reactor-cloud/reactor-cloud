//! Reactor Jobs — durable execution with step checkpointing
//!
//! This crate implements the Jobs capability per `docs/reactor-jobs.design.md`:
//! - Durable execution on top of `reactor-functions`
//! - Step checkpointing with cached replay
//! - Multiple trigger types: cron, webhook, event, manual
//! - TypeScript SDK support
//! - Retry with exponential backoff and DLQ

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod config;
pub mod error;
pub mod manifest;
pub mod middleware;
pub mod router;
pub mod routes;
pub mod scheduler;
pub mod state;
pub mod store;
pub mod worker;

use once_cell::sync::Lazy;
use regex::Regex;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Regex for valid job names (lowercase alphanumeric with hyphens, 3-63 chars).
pub static JOB_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9-]{1,61}[a-z0-9]$").expect("invalid job name regex")
});

/// Regex for valid event topic names (lowercase alphanumeric with dots, 3-128 chars).
pub static EVENT_TOPIC_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9.]{1,126}[a-z0-9]$").expect("invalid event topic regex")
});

// Re-exports
pub use config::{Deployment, JobsConfig};
pub use error::JobsError;
pub use manifest::{JobManifest, RetryConfig, TriggerConfig, TriggerKind};
pub use router::router;
pub use state::{JobCtx, JobsState};
pub use store::{
    DlqEntry, Event, Job, JobId, JobsStore, JobsTx, NewEvent, NewJob, NewRun, NewStep, NewTrigger,
    PgJobsStore, Run, RunId, RunStatus, StateEntry, Step, StepId, StepStatus, Trigger, TriggerId,
};

use utoipa::OpenApi;

/// OpenAPI documentation for the jobs service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor Jobs API",
        version = "1.0.0",
        description = "Durable execution with step checkpointing"
    ),
    paths(
        routes::health::health,
    ),
    components(schemas(
        routes::health::HealthResponse,
    )),
    tags(
        (name = "jobs", description = "Job management and execution"),
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

/// Returns the OpenAPI specification for the jobs service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
