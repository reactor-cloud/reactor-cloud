//! Router construction for reactor-analytics.

use crate::middleware::auth::require_auth_middleware;
use crate::middleware::project_key::project_key_middleware;
use crate::routes;
use crate::state::AnalyticsState;
use crate::store::AnalyticsStore;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

/// Build the reactor-analytics router.
///
/// All routes are prefixed with `/analytics/v1`.
///
/// Route authentication:
/// - Public: /health, /openapi.json
/// - Project key (anonymous): /track, /batch, /identify, /alias, /consent/*
/// - Bearer JWT (authenticated): /projects/*, /query, /erase, /export
pub fn router<S: AnalyticsStore + Clone>(state: AnalyticsState<S>) -> Router {
    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(routes::health::<S>))
        .route("/metrics", get(routes::metrics::metrics::<S>))
        .route("/snippet.js", get(routes::snippet::snippet))
        .route("/openapi.json", get(openapi_handler));

    // Ingestion routes (project key authentication)
    let ingest_routes: Router<AnalyticsState<S>> = Router::new()
        .route("/track", post(routes::ingest::track::<S>))
        .route("/batch", post(routes::ingest::batch::<S>))
        .route("/identify", post(routes::ingest::identify::<S>))
        .route("/alias", post(routes::ingest::alias::<S>))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            project_key_middleware::<S>,
        ));

    // Consent routes (project key authentication)
    let consent_routes: Router<AnalyticsState<S>> = Router::new()
        .route("/consent/opt-out", post(routes::consent::opt_out::<S>))
        .route("/consent/opt-in", post(routes::consent::opt_in::<S>))
        .route("/consent/status", post(routes::consent::status::<S>))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            project_key_middleware::<S>,
        ));

    // Admin routes (require bearer JWT authentication)
    let admin_routes = Router::new()
        .route("/projects", post(routes::admin::create_project::<S>))
        .route("/projects", get(routes::admin::list_projects::<S>))
        .route("/projects/{project_id}", get(routes::admin::get_project::<S>))
        .route("/projects/{project_id}", delete(routes::admin::delete_project::<S>))
        .route(
            "/projects/{project_id}/keys",
            post(routes::admin::create_project_key::<S>),
        )
        .route(
            "/projects/{project_id}/keys",
            get(routes::admin::list_project_keys::<S>),
        )
        .route(
            "/projects/{project_id}/keys/{key_id}",
            delete(routes::admin::revoke_project_key::<S>),
        )
        // GDPR routes
        .route("/erase", post(routes::erasure::erase::<S>))
        .route("/export", post(routes::erasure::export::<S>))
        // Query endpoint
        .route("/query", post(routes::query::query::<S>))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_auth_middleware::<S>,
        ));

    let api = Router::new()
        .merge(public_routes)
        .merge(ingest_routes)
        .merge(consent_routes)
        .merge(admin_routes);

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
        .nest("/analytics/v1", api)
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
