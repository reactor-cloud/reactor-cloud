//! Health check endpoint.

use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

/// Health response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Health status.
    pub status: String,
    /// Server version.
    pub version: String,
    /// Enabled frameworks.
    pub frameworks: Vec<String>,
}

/// Health check handler.
#[utoipa::path(
    get,
    path = "/sites/v1/health",
    tag = "sites",
    responses(
        (status = 200, description = "Health status", body = HealthResponse),
    )
)]
pub async fn health() -> Json<HealthResponse> {
    let frameworks = crate::enabled_frameworks()
        .iter()
        .map(|f| f.to_string())
        .collect();

    Json(HealthResponse {
        status: "ok".to_string(),
        version: crate::VERSION.to_string(),
        frameworks,
    })
}
