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

/// GET /auth/v1/health
///
/// Returns the health status of the auth service.
#[utoipa::path(
    get,
    path = "/auth/v1/health",
    tag = "auth",
    responses(
        (status = 200, description = "Health status", body = HealthResponse),
    )
)]
pub async fn health() -> impl IntoResponse {
    let response = HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    };
    (StatusCode::OK, Json(response))
}
