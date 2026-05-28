//! Router construction for reactor-storage.

use crate::middleware::{auth_middleware, verify_signed_url_middleware};
use crate::routes;
use crate::state::StorageState;
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

/// Build the reactor-storage router.
///
/// All routes are prefixed with `/storage/v1`.
pub fn router(state: StorageState) -> Router {
    // Health route (no auth required)
    let health_route = Router::new()
        .route("/health", get(routes::health))
        .route("/openapi.json", get(openapi_handler));

    // Metrics route (no auth required, gated by config)
    let metrics_route = Router::new().route("/metrics", get(routes::metrics));

    // Bucket routes (auth required via middleware)
    let bucket_routes = Router::new()
        .route(
            "/buckets",
            get(routes::list_buckets).post(routes::create_bucket),
        )
        .route(
            "/buckets/:bucket_ref",
            get(routes::get_bucket)
                .patch(routes::update_bucket)
                .delete(routes::delete_bucket),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Object routes (auth via middleware, allows anonymous for public buckets)
    // Signed URL verification runs first, then auth for non-signed requests
    // Note: *key captures the full path including slashes
    #[cfg(any(feature = "fs", feature = "s3"))]
    let object_routes = Router::new()
        .route(
            "/object/:bucket/*key",
            get(routes::get_object)
                .put(routes::put_object)
                .head(routes::head_object)
                .delete(routes::delete_object),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            verify_signed_url_middleware,
        ));

    // Signed URL generation routes (requires auth)
    #[cfg(any(feature = "fs", feature = "s3"))]
    let sign_routes = Router::new()
        .route(
            "/sign/:bucket/*key",
            axum::routing::post(routes::create_signed_url),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Multipart upload routes (key must not contain slashes for multipart)
    #[cfg(any(feature = "fs", feature = "s3"))]
    let multipart_routes = Router::new()
        .route(
            "/upload/:bucket/:key",
            axum::routing::post(routes::create_multipart_upload),
        )
        .route(
            "/upload/:bucket/:key/part",
            axum::routing::put(routes::upload_part),
        )
        .route(
            "/upload/:bucket/:key/complete",
            axum::routing::post(routes::complete_multipart_upload),
        )
        .route(
            "/upload/:bucket/:key/abort",
            axum::routing::delete(routes::abort_multipart_upload),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    #[cfg(any(feature = "fs", feature = "s3"))]
    let api = health_route
        .merge(metrics_route)
        .merge(bucket_routes)
        .merge(object_routes)
        .merge(multipart_routes)
        .merge(sign_routes);

    #[cfg(not(any(feature = "fs", feature = "s3")))]
    let api = health_route.merge(metrics_route).merge(bucket_routes);

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
        .nest("/storage/v1", api)
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
