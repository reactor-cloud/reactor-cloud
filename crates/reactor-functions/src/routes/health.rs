//! Health check endpoint.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use utoipa::ToSchema;

/// Health check response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Service status.
    pub status: &'static str,
    /// Service version.
    pub version: &'static str,
    /// Enabled runtime adapters.
    pub runtimes: Vec<&'static str>,
}

/// GET /fn/v1/health
///
/// Returns service health status and enabled runtimes.
#[utoipa::path(
    get,
    path = "/fn/v1/health",
    tag = "functions",
    responses(
        (status = 200, description = "Health status", body = HealthResponse),
    )
)]
pub async fn health() -> impl IntoResponse {
    let response = HealthResponse {
        status: "ok",
        version: crate::VERSION,
        runtimes: enabled_runtimes(),
    };
    (StatusCode::OK, Json(response))
}

/// Returns the list of enabled runtime adapters.
pub fn enabled_runtimes() -> Vec<&'static str> {
    let mut runtimes = Vec::new();

    #[cfg(feature = "runtime-wasm")]
    runtimes.push("wasm");

    #[cfg(feature = "runtime-bun")]
    runtimes.push("bun");

    #[cfg(feature = "runtime-lambda")]
    runtimes.push("lambda");

    runtimes
}
