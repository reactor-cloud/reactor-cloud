//! Admin endpoint handlers.
//!
//! The `/_admin/*` endpoints provide deployment, migration, doctor, and shutdown
//! functionality. All routes (except health/metrics/openapi) require the admin bearer token.

pub mod auth;
pub mod deploy;
pub mod doctor;
pub mod health;
pub mod metrics;
pub mod migrate;
pub mod openapi;
pub mod shutdown;
pub mod vault;
pub mod version;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, post, put},
    Router,
};

pub use auth::AdminAuthState;
pub use vault::VaultState;

/// Build the admin router.
///
/// Routes without auth:
/// - `GET /health` — Composite health check
/// - `GET /metrics` — Prometheus metrics
///
/// Routes with admin auth:
/// - `GET /_admin/version` — Version info
/// - `POST /_admin/migrate` — Run migrations
/// - `GET /_admin/doctor` — Health probes
/// - `POST /_admin/deploy` — Deploy bundle (up to 256MB)
/// - `POST /_admin/shutdown` — Trigger shutdown
/// - `GET /_admin/vault/secrets` — List secrets
/// - `GET /_admin/vault/secrets/:key` — Get secret
/// - `PUT /_admin/vault/secrets/:key` — Set secret
/// - `DELETE /_admin/vault/secrets/:key` — Delete secret
/// - `POST /_admin/vault/rotate` — Rotate transit key
pub fn router(admin_state: AdminAuthState) -> Router {
    // Public routes (no auth)
    let public = Router::new()
        .route("/health", get(health::health_handler))
        .route("/metrics", get(metrics::metrics_handler))
        .route("/_api/openapi.json", get(openapi::openapi_handler));

    // Deploy route needs larger body limit for bundles (256MB)
    let deploy_route = Router::new()
        .route("/_admin/deploy", post(deploy::deploy_handler))
        .layer(DefaultBodyLimit::max(256 * 1024 * 1024));

    // Vault routes
    let vault_routes = Router::new()
        .route("/_admin/vault/secrets", get(vault::list_secrets_handler))
        .route(
            "/_admin/vault/secrets/:key",
            get(vault::get_secret_handler)
                .put(vault::set_secret_handler)
                .delete(vault::delete_secret_handler),
        )
        .route("/_admin/vault/rotate", post(vault::rotate_key_handler));

    // Other admin routes (default 2MB limit)
    let admin = Router::new()
        .route("/_admin/version", get(version::version_handler))
        .route("/_admin/migrate", post(migrate::migrate_handler))
        .route("/_admin/doctor", get(doctor::doctor_handler))
        .route("/_admin/shutdown", post(shutdown::shutdown_handler))
        .merge(deploy_route)
        .merge(vault_routes)
        .with_state(admin_state.clone())
        .layer(middleware::from_fn_with_state(
            admin_state,
            auth::admin_auth_middleware,
        ));

    public.merge(admin)
}
