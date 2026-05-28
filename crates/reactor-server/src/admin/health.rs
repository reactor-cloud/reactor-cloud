//! Composite health check endpoint.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Overall status: "healthy" or "unhealthy".
    pub status: String,

    /// Per-capability health status.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<CapabilityHealth>,
}

/// Per-capability health status.
#[derive(Debug, Serialize)]
pub struct CapabilityHealth {
    /// Capability name.
    pub name: String,

    /// Status: "healthy" or "unhealthy".
    pub status: String,

    /// Optional error message if unhealthy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Health check handler.
///
/// Returns 200 if all capabilities are healthy, 503 if any are unhealthy.
pub async fn health_handler() -> impl IntoResponse {
    // In a full implementation, this would fan out to each capability's
    // health check endpoint. For now, we return a basic healthy response.
    let response = HealthResponse {
        status: "healthy".to_string(),
        capabilities: vec![],
    };

    (StatusCode::OK, Json(response))
}
