//! Axum router factory for the Connect service.

use crate::routes;
use crate::state::ConnectState;
use crate::store::ConnectStore;
use axum::{routing::get, Router};

/// Create the Connect service router.
///
/// All routes are prefixed with `/connect/v1`.
pub fn router<S: ConnectStore + Clone + Send + Sync + 'static>(state: ConnectState<S>) -> Router {
    Router::new()
        // Health check
        .route("/connect/v1/health", get(routes::health::health))
        // Catalog routes
        .route("/connect/v1/catalog", get(routes::catalog::list))
        .route("/connect/v1/catalog/:type_id", get(routes::catalog::show))
        // Instance routes
        .route(
            "/connect/v1/instances",
            get(routes::instances::list).post(routes::instances::create),
        )
        .route(
            "/connect/v1/instances/:name",
            get(routes::instances::show).delete(routes::instances::delete),
        )
        .route(
            "/connect/v1/instances/:name/credentials",
            axum::routing::post(routes::instances::credentials),
        )
        .route(
            "/connect/v1/instances/:name/check",
            axum::routing::post(routes::instances::check),
        )
        .route(
            "/connect/v1/instances/:name/discover",
            axum::routing::post(routes::instances::discover),
        )
        // Action routes
        .route(
            "/connect/v1/instances/:name/actions/:action/invoke",
            axum::routing::post(routes::invoke::invoke_action),
        )
        .route(
            "/connect/v1/instances/:name/actions/:action/sandbox",
            axum::routing::post(routes::sandbox::action_sandbox),
        )
        // Receiver routes
        .route(
            "/connect/v1/instances/:name/receivers",
            get(routes::receivers::list).post(routes::receivers::create),
        )
        .route(
            "/connect/v1/instances/:name/receivers/:receiver_id",
            get(routes::receivers::show).delete(routes::receivers::delete),
        )
        // Webhook ingress (anonymous)
        .route(
            "/connect/v1/ingress/:receiver_token",
            axum::routing::post(routes::ingress::webhook_ingress),
        )
        // Connection routes (v0.2+, stubs for now)
        .route(
            "/connect/v1/connections",
            get(routes::connections::list).post(routes::connections::create),
        )
        .route(
            "/connect/v1/connections/:name",
            get(routes::connections::show)
                .patch(routes::connections::update)
                .delete(routes::connections::delete),
        )
        .with_state(state)
}
