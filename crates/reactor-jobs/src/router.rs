//! Router configuration for reactor-jobs.

use axum::{
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};

use crate::middleware::auth::require_auth;
use crate::routes;
use crate::state::JobsState;

/// Create the jobs router.
pub fn router(state: JobsState) -> Router {
    // Admin routes (require auth)
    let admin_routes = Router::new()
        // Job CRUD
        .route("/jobs", post(routes::admin::create_job))
        .route("/jobs", get(routes::admin::list_jobs))
        .route("/jobs/{name}", get(routes::admin::get_job))
        .route("/jobs/{name}", delete(routes::admin::delete_job))
        // Trigger CRUD
        .route("/jobs/{name}/triggers", post(routes::triggers::create_trigger))
        .route("/jobs/{name}/triggers", get(routes::triggers::list_triggers))
        .route(
            "/jobs/{name}/triggers/{trigger_id}",
            delete(routes::triggers::delete_trigger),
        )
        // Run management
        .route("/jobs/{name}/runs", get(routes::runs::list_runs))
        .route("/jobs/{name}/runs/{run_id}", get(routes::runs::get_run))
        .route(
            "/jobs/{name}/runs/{run_id}/cancel",
            post(routes::runs::cancel_run),
        )
        .route(
            "/jobs/{name}/runs/{run_id}/retry",
            post(routes::runs::retry_run),
        )
        // DLQ management
        .route("/jobs/{name}/dlq", get(routes::dlq::list_dlq))
        .route("/jobs/{name}/dlq/{dlq_id}/retry", post(routes::dlq::retry_dlq))
        .route("/jobs/{name}/dlq/{dlq_id}", delete(routes::dlq::delete_dlq))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // Trigger routes (require auth)
    let trigger_routes = Router::new()
        .route("/{name}/trigger", post(routes::invoke::manual_trigger))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // Webhook routes (no auth - token is self-contained)
    let webhook_routes = Router::new().route("/webhooks/{token}", post(routes::invoke::webhook_trigger));

    // Internal routes (for SDK communication, requires internal auth)
    let internal_routes = Router::new()
        .route("/steps", post(routes::internal::create_step))
        .route("/steps/{step_id}", axum::routing::put(routes::internal::update_step))
        .route("/state", post(routes::internal::set_state))
        .route("/state/{key}", get(routes::internal::get_state))
        .route("/state/{key}", delete(routes::internal::delete_state))
        .route("/events", post(routes::internal::emit_event))
        .route("/sleep", post(routes::internal::request_sleep));

    // Logs route (under admin, requires auth)
    let logs_routes = Router::new()
        .route("/jobs/{name}/logs", get(routes::logs::stream_logs))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        // Health check (no auth)
        .route("/jobs/v1/health", get(routes::health::health))
        // OpenAPI spec (no auth)
        .route("/jobs/v1/openapi.json", get(openapi_handler))
        // Metrics endpoint (no auth, gated by config)
        .route("/jobs/v1/metrics", get(routes::metrics::metrics))
        // Admin routes
        .nest("/jobs/v1/_admin", admin_routes)
        // Logs routes
        .nest("/jobs/v1/_admin", logs_routes)
        // Trigger routes
        .nest("/jobs/v1", trigger_routes)
        // Webhook routes
        .nest("/jobs/v1", webhook_routes)
        // Internal routes (for SDK)
        .nest("/jobs/v1/_internal", internal_routes)
        .with_state(state)
}

/// Handler for the OpenAPI specification endpoint.
async fn openapi_handler() -> impl IntoResponse {
    let spec = crate::openapi();
    (StatusCode::OK, Json(spec))
}
