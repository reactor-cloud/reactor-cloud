//! Router construction for reactor-ai.

use crate::routes;
use crate::state::AiState;
use axum::{
    body::Body,
    http::{Request, StatusCode},
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

/// Build the reactor-ai router.
///
/// All routes are prefixed with `/ai/v1`.
pub fn router(state: AiState) -> Router {
    // Health route (no auth required)
    let health_routes = Router::new()
        .route("/health", get(routes::health::health))
        .route("/openapi.json", get(openapi_handler));

    // OpenAI-compatible routes (auth required via reactor-core middleware at composition)
    let inference_routes = Router::new()
        .route("/chat/completions", post(routes::chat::chat_completions))
        .route("/embeddings", post(routes::embeddings::embeddings))
        .route("/models", get(routes::models::list_models));

    let api = health_routes.merge(inference_routes);

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
        .nest("/ai/v1", api)
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
