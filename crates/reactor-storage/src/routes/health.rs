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
}

/// GET /storage/v1/health
///
/// Returns service health status.
#[utoipa::path(
    get,
    path = "/storage/v1/health",
    tag = "storage",
    responses(
        (status = 200, description = "Health status", body = HealthResponse),
    )
)]
pub async fn health() -> impl IntoResponse {
    let response = HealthResponse {
        status: "ok",
        version: crate::VERSION,
    };
    (StatusCode::OK, Json(response))
}
