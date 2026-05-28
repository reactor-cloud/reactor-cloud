//! Reactor Analytics capability.
//!
//! Provides product analytics for the Reactor BaaS: event ingestion, identity
//! stitching, funnel/retention/path queries, and GDPR compliance.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod client;
pub mod config;
pub mod error;
pub mod ingest;
pub mod middleware;
pub mod observability;
pub mod query;
pub mod router;
pub mod routes;
pub mod state;
pub mod store;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use client::{AnalyticsClient, AnalyticsClientBuilder, AnalyticsClientConfig, TrackEvent};
pub use config::AnalyticsConfig;
pub use error::AnalyticsError;
pub use router::router;
pub use state::{AnalyticsCtx, AnalyticsState};
pub use store::{AnalyticsStore, PgAnalyticsStore};

use utoipa::OpenApi;

/// OpenAPI documentation for the analytics service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor Analytics API",
        version = "1.0.0",
        description = "Product analytics API for Reactor"
    ),
    paths(
        routes::health::health,
    ),
    components(schemas(
        routes::health::HealthResponse,
    )),
    tags(
        (name = "analytics", description = "Core analytics operations"),
        (name = "analytics.ingest", description = "Event ingestion"),
        (name = "analytics.query", description = "Analytics queries"),
        (name = "analytics.admin", description = "Project and key management"),
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
            components.add_security_scheme(
                "project_key",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Header(
                        utoipa::openapi::security::ApiKeyValue::new("X-Reactor-Project-Key"),
                    ),
                ),
            );
        }
    }
}

/// Returns the OpenAPI specification for the analytics service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
