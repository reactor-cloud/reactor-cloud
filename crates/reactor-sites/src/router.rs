//! Router construction for reactor-sites.

use crate::middleware::auth_middleware;
use crate::routes;
use crate::state::SitesState;
use axum::{
    body::Body, 
    http::{Request, StatusCode}, 
    middleware, 
    response::IntoResponse,
    routing::get, 
    Json, Router,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

/// Build the serve plane router.
///
/// This router serves as a fallback for the unified server, routing
/// requests that don't match API prefixes to the sites serve handler
/// based on the Host header.
pub fn serve_router(state: SitesState) -> Router {
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &Request<Body>| {
            let request_id = request
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-");
            let host = request
                .headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-");

            tracing::info_span!(
                "serve_request",
                method = %request.method(),
                uri = %request.uri(),
                host = %host,
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
        .fallback(routes::serve_handler)
        .with_state(state)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(trace_layer)
}

/// Build the reactor-sites router.
///
/// All routes are prefixed with `/sites/v1`.
pub fn router(state: SitesState) -> Router {
    let health_route = Router::new()
        .route("/health", get(routes::health))
        .route("/openapi.json", get(openapi_handler));

    let admin_routes = Router::new()
        .route(
            "/_admin/sites",
            get(routes::list_sites).post(routes::create_site),
        )
        .route(
            "/_admin/sites/:name",
            get(routes::get_site).delete(routes::delete_site),
        )
        .route(
            "/_admin/sites/:name/deployments",
            get(routes::list_deployments).post(routes::create_deployment),
        )
        .route(
            "/_admin/sites/:name/deployments/:deployment_id",
            get(routes::get_deployment),
        )
        .route(
            "/_admin/sites/:name/promote",
            axum::routing::post(routes::promote_deployment),
        )
        .route(
            "/_admin/sites/:name/rollback",
            axum::routing::post(routes::rollback_deployment),
        )
        .route(
            "/_admin/sites/:name/domains",
            get(routes::list_domains).post(routes::create_domain),
        )
        .route(
            "/_admin/sites/:name/domains/:host",
            axum::routing::delete(routes::delete_domain),
        )
        .route(
            "/_admin/sites/:name/domains/:host/verify",
            axum::routing::post(routes::verify_domain),
        )
        .route(
            "/_admin/sites/:name/revalidate",
            axum::routing::post(routes::revalidate),
        )
        .route(
            "/_admin/sites/:name/logs",
            get(routes::stream_logs),
        )
        .route(
            "/_admin/sites/:name/policies",
            get(routes::list_policies).post(routes::create_policy),
        )
        .route(
            "/_admin/sites/:name/policies/:policy_name",
            axum::routing::delete(routes::delete_policy),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let metrics_routes = Router::new().route("/metrics", get(routes::metrics_handler));

    let api = health_route.merge(admin_routes).merge(metrics_routes);

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
        .nest("/sites/v1", api)
        .with_state(state)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(trace_layer)
}

/// Handler for the OpenAPI specification endpoint.
async fn openapi_handler() -> impl IntoResponse {
    let spec = crate::openapi();
    (StatusCode::OK, Json(spec))
}
