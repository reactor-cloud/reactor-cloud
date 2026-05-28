//! Reactor Sites capability — app hosting as an orchestration layer over functions and storage
//!
//! This crate implements the Sites capability per `docs/reactor-sites.design.md`:
//! - Static file serving via `reactor-storage`
//! - SSR function dispatch via `reactor-functions`
//! - Preview deployments with auto-subdomains
//! - ISR (Incremental Static Regeneration) with on-demand revalidation
//! - Custom domains with ACME TLS provisioning
//! - Per-site policy enforcement via `reactor-policy`
//!
//! Framework adapters: `static`, `hono`, `nextjs`

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod bundle;
pub mod config;
pub mod dispatch;
pub mod domain;
pub mod error;
#[cfg(any(
    feature = "framework-static",
    feature = "framework-hono",
    feature = "framework-nextjs"
))]
pub mod framework;
pub mod isr;
pub mod middleware;
pub mod route;
pub mod router;
pub mod routes;
pub mod state;
pub mod store;

use once_cell::sync::Lazy;
use regex::Regex;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Regex for valid site names (lowercase alphanumeric with hyphens, 1-63 chars).
pub static SITE_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9-]{0,62}$").expect("invalid site name regex")
});

/// Framework types supported by reactor-sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    /// Static file hosting (HTML/CSS/JS only).
    Static,
    /// Astro static site generator.
    Astro,
    /// Hono SSR framework.
    Hono,
    /// Next.js App Router.
    Nextjs,
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Framework::Static => write!(f, "static"),
            Framework::Astro => write!(f, "astro"),
            Framework::Hono => write!(f, "hono"),
            Framework::Nextjs => write!(f, "nextjs"),
        }
    }
}

impl std::str::FromStr for Framework {
    type Err = error::SitesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "static" => Ok(Framework::Static),
            "astro" => Ok(Framework::Astro),
            "hono" => Ok(Framework::Hono),
            "nextjs" => Ok(Framework::Nextjs),
            _ => Err(error::SitesError::InvalidFramework(s.to_string())),
        }
    }
}

// Re-exports
pub use config::{Deployment, SitesConfig};
pub use error::SitesError;
pub use router::{router, serve_router};
pub use routes::HealthResponse;
pub use state::{SiteCtx, SitesState};
pub use store::{
    AuditEvent, AuditEventCreate, AuditEventId, DeploymentStatus, Domain, DomainId, DomainStatus,
    IsrCacheEntry, NewDeployment, NewDomain, NewSite, NewSitePolicy, PgSitesStore, Site,
    SiteDeployment, SiteDeploymentId, SiteId, SitePolicy, SitesStore,
};

/// Get the list of enabled frameworks based on compile-time features.
pub fn enabled_frameworks() -> Vec<Framework> {
    let mut frameworks = Vec::new();

    #[cfg(feature = "framework-static")]
    frameworks.push(Framework::Static);

    #[cfg(feature = "framework-hono")]
    frameworks.push(Framework::Hono);

    #[cfg(feature = "framework-nextjs")]
    frameworks.push(Framework::Nextjs);

    frameworks
}

use utoipa::OpenApi;

/// OpenAPI documentation for the sites service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor Sites API",
        version = "1.0.0",
        description = "App hosting with static files and SSR"
    ),
    paths(
        routes::health::health,
    ),
    components(schemas(
        routes::HealthResponse,
    )),
    tags(
        (name = "sites", description = "Site management and deployment"),
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

/// Returns the OpenAPI specification for the sites service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
