//! Health check endpoint.

use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use crate::VERSION;

/// Health check response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Service status.
    pub status: &'static str,
    /// Service version.
    pub version: &'static str,
    /// Scheduler status.
    pub scheduler: &'static str,
    /// Number of workers.
    pub workers: usize,
}

/// Health check handler.
#[utoipa::path(
    get,
    path = "/jobs/v1/health",
    tag = "jobs",
    responses(
        (status = 200, description = "Health status", body = HealthResponse),
    )
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: VERSION,
        scheduler: "running",
        workers: 4, // TODO: Get from state
    })
}
