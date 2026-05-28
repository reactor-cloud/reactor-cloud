//! Router construction for reactor-functions.

use crate::middleware::auth_middleware;
use crate::policy::routes as policy_routes;
use crate::routes;
use crate::state::FunctionsState;
use axum::{
    body::Body, 
    http::{Request, StatusCode}, 
    middleware, 
    response::IntoResponse,
    routing::{any, get}, 
    Json, Router,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

/// Build the reactor-functions router.
///
/// All routes are prefixed with `/fn/v1`.
pub fn router(state: FunctionsState) -> Router {
    // Health route (no auth required)
    let health_route = Router::new()
        .route("/health", get(routes::health))
        .route("/openapi.json", get(openapi_handler));

    // Admin routes (auth required)
    let admin_routes = Router::new()
        .route(
            "/_admin/functions",
            get(routes::list_functions).post(routes::create_function),
        )
        .route(
            "/_admin/functions/{name}",
            get(routes::get_function).delete(routes::delete_function),
        )
        .route(
            "/_admin/functions/{name}/deployments",
            get(routes::list_deployments).post(routes::create_deployment),
        )
        .route(
            "/_admin/functions/{name}/deployments/{deployment_id}",
            get(routes::get_deployment),
        )
        .route(
            "/_admin/functions/{name}/env",
            get(routes::list_env),
        )
        .route(
            "/_admin/functions/{name}/env/{key}",
            get(routes::get_env)
                .put(routes::set_env)
                .delete(routes::delete_env),
        )
        .route(
            "/_admin/functions/{name}/policies",
            get(policy_routes::list_policies)
                .post(policy_routes::create_policy),
        )
        .route(
            "/_admin/functions/{name}/policies/{policy_name}",
            axum::routing::delete(policy_routes::delete_policy),
        )
        .route(
            "/_admin/functions/{name}/promote",
            axum::routing::post(routes::promote_deployment),
        )
        .route(
            "/_admin/functions/{name}/rollback",
            axum::routing::post(routes::rollback_deployment),
        )
        .route(
            "/_admin/functions/{name}/logs",
            get(routes::stream_logs),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Invoke routes (auth required, any HTTP method)
    // Matches /fn/v1/{name} and /fn/v1/{name}/*rest
    let invoke_routes = Router::new()
        .route("/{name}", any(routes::invoke_handler))
        .route("/{name}/*rest", any(routes::invoke_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Metrics route (no auth required)
    let metrics_routes = Router::new()
        .route("/metrics", get(routes::metrics_handler));

    let api = health_route
        .merge(admin_routes)
        .merge(invoke_routes)
        .merge(metrics_routes);

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
        .nest("/fn/v1", api)
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
