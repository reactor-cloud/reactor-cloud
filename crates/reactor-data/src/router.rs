//! Router construction for reactor-data.

use crate::middleware::auth_middleware;
use crate::routes;
use crate::state::DataState;
use crate::store::DataStore;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

/// Build the reactor-data router.
///
/// All routes are prefixed with `/data/v1`.
pub fn router<S: DataStore + Clone + 'static>(state: DataState<S>) -> Router {
    // Health route (no auth required)
    let health_route = Router::new()
        .route("/health", get(routes::health))
        .route("/openapi.json", get(openapi_handler));

    // Metrics route (no auth required, gated by config)
    let metrics_route = if state.config.metrics {
        Router::new().route("/metrics", get(routes::metrics))
    } else {
        Router::new()
    };

    // CRUD routes (auth required)
    let crud_routes = Router::new()
        .route(
            "/:table",
            get(routes::get_table::<S>)
                .post(routes::post_table::<S>)
                .patch(routes::patch_table::<S>)
                .delete(routes::delete_table::<S>),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware::<S>,
        ));

    // RPC routes (auth required)
    let rpc_routes = Router::new()
        .route("/rpc/:name", post(routes::post_rpc::<S>))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware::<S>,
        ));

    // Admin routes (auth required)
    let admin_routes = Router::new()
        .route("/_admin/types/typescript", get(routes::generate_typescript::<S>))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware::<S>,
        ));

    let api = health_route
        .merge(metrics_route)
        .merge(crud_routes)
        .merge(rpc_routes)
        .merge(admin_routes);

    // Build the trace layer with structured logging
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &Request<Body>| {
            let request_id = request
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-");

            tracing::info_span!(
                "http_request",
                method = %request.method(),
                uri = %request.uri(),
                request_id = %request_id,
            )
        })
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(LatencyUnit::Millis),
        );

    Router::new()
        .nest("/data/v1", api)
        .with_state(state)
        // Add request ID middleware (set if missing, propagate to response)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        // Add tracing layer
        .layer(trace_layer)
}

/// Handler for the OpenAPI specification endpoint.
async fn openapi_handler() -> impl IntoResponse {
    let spec = crate::openapi();
    (StatusCode::OK, Json(spec))
}
