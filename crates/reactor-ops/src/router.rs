//! Router configuration for the ops control surface.

use crate::audit::AuditLogger;
use crate::middleware::{
    audit_log, identity_check, network_check, scope_check, step_up_check,
    OpsMiddlewareState, RouteMeta,
};
use crate::routes;
use crate::state::OpsState;
use axum::{
    middleware::from_fn_with_state,
    routing::{delete, get, post},
    Router,
};

/// Create the ops router.
///
/// All routes are prefixed with `/_ops/v1`.
pub fn router(state: OpsState) -> Router {
    let config = state.config.clone();
    let auth = state.auth.clone();
    let audit = AuditLogger::new(state.pool.clone());

    let middleware_state = OpsMiddlewareState {
        auth: auth.clone(),
        config: config.clone(),
        audit,
    };

    // Operators routes (bootstrap is special - only needs network check)
    let operators_routes = Router::new()
        .route(
            "/operators/bootstrap",
            post(routes::operators::bootstrap)
                .layer(axum::Extension(RouteMeta::new("ops:bootstrap", "operators.bootstrap"))),
        )
        .route(
            "/operators/status",
            get(routes::operators::status)
                .layer(axum::Extension(RouteMeta::new("ops:read", "operators.status"))),
        )
        .route(
            "/operators/promote",
            post(routes::operators::promote)
                .layer(axum::Extension(RouteMeta::new("ops:cluster_admin", "operators.promote"))),
        );

    // Health/status routes (minimal auth)
    let health_routes = Router::new()
        .route(
            "/doctor",
            get(routes::health::doctor)
                .layer(axum::Extension(RouteMeta::new("ops:read", "doctor"))),
        )
        .route(
            "/version",
            get(routes::health::version)
                .layer(axum::Extension(RouteMeta::new("ops:read", "version"))),
        );

    // Deployment routes (status only - the POST handler is provided by
    // `reactor-server::compose::ops` because the real implementation needs
    // access to the multipart bundle pipeline that lives there).
    let deployment_routes = Router::new()
        .route(
            "/deployments/status",
            get(routes::deployments::deployment_status)
                .layer(axum::Extension(RouteMeta::new("ops:deploy", "deployments.status"))),
        );

    // Audit routes
    let audit_routes = Router::new()
        .route(
            "/audit",
            get(routes::audit::list_audit)
                .layer(axum::Extension(RouteMeta::new("ops:read", "audit.list"))),
        );

    // Projects routes are provided by `reactor-server::compose::ops` so they
    // can delegate to `CloudApiState` without creating a circular crate
    // dependency. This crate exposes the placeholder handlers for tests but
    // does not wire them into the router.
    let projects_routes: Router<OpsState> = Router::new();

    // Vault routes
    let vault_routes = Router::new()
        .route(
            "/vault/*path",
            get(routes::vault::read_secret)
                .layer(axum::Extension(RouteMeta::new("vault:read", "vault.read"))),
        )
        .route(
            "/vault/*path",
            axum::routing::put(routes::vault::write_secret)
                .layer(axum::Extension(RouteMeta::new("vault:write", "vault.write"))),
        )
        .route(
            "/vault/*path",
            delete(routes::vault::delete_secret)
                .layer(axum::Extension(RouteMeta::new("vault:write", "vault.delete"))),
        );

    // Combine all routes
    let api = Router::new()
        .merge(operators_routes)
        .merge(health_routes)
        .merge(deployment_routes)
        .merge(projects_routes)
        .merge(vault_routes)
        .merge(audit_routes)
        .with_state(state.clone());

    // Apply middleware stack
    // Note: middleware runs in reverse order of how they're added
    let api = api
        .layer(from_fn_with_state(middleware_state.clone(), audit_log))
        .layer(from_fn_with_state(config.clone(), step_up_check))
        .layer(from_fn_with_state((), scope_check))
        .layer(from_fn_with_state(auth.clone(), identity_check))
        .layer(from_fn_with_state(config.clone(), network_check));

    Router::new().nest("/_ops/v1", api)
}
