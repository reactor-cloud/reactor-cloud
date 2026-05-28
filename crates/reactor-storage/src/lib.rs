//! Reactor Storage capability.
//!
//! Provides S3-shaped HTTP surface with dual FS/S3 backends for blob storage.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod config;
pub mod error;
pub mod middleware;
pub mod policy;
pub mod router;
pub mod routes;
pub mod state;
pub mod store;

/// Feature-gated modules for backend adapters.
#[cfg(feature = "fs")]
pub mod fs;
#[cfg(feature = "fs")]
pub use fs::FsBlobStore;

#[cfg(feature = "s3")]
pub mod s3;
#[cfg(feature = "s3")]
pub use s3::S3BlobStore;

use once_cell::sync::Lazy;
use regex::Regex;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Regex for valid bucket slugs (lowercase alphanumeric with hyphens, not starting/ending with hyphen).
pub static SLUG_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?$").expect("invalid slug regex")
});

// Re-exports for convenience
pub use audit::{AuditAction, AuditEvent, AuditEventBuilder};
pub use config::{Deployment, StorageConfig};
pub use error::StorageError;
pub use router::router;
pub use state::StorageState;
pub use store::{BlobStore, MetadataStore, PgMetadataStore};

use utoipa::OpenApi;

/// OpenAPI documentation for the storage service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor Storage API",
        version = "1.0.0",
        description = "S3-shaped blob storage API"
    ),
    paths(
        routes::buckets::create_bucket,
        routes::buckets::list_buckets,
        routes::buckets::get_bucket,
        routes::buckets::update_bucket,
        routes::buckets::delete_bucket,
        routes::health::health,
    ),
    components(schemas(
        routes::buckets::CreateBucketRequest,
        routes::buckets::UpdateBucketRequest,
        routes::buckets::BucketResponse,
        routes::buckets::BucketsListResponse,
        routes::health::HealthResponse,
    )),
    tags(
        (name = "storage", description = "Core storage operations"),
        (name = "storage.buckets", description = "Bucket management"),
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

/// Returns the OpenAPI specification for the storage service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
