//! Health check endpoint.

use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Health check response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    /// Service status.
    pub status: String,
    /// Service version.
    pub version: String,
    /// Enabled runtimes.
    pub runtimes: Vec<String>,
}

/// GET /connect/v1/health
#[utoipa::path(
    get,
    path = "/connect/v1/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    ),
    tag = "connect"
)]
pub async fn health() -> Json<HealthResponse> {
    let mut runtimes = vec![];
    
    #[cfg(feature = "runtime-native")]
    runtimes.push("native".to_string());
    
    #[cfg(feature = "runtime-manifest")]
    runtimes.push("manifest".to_string());
    
    #[cfg(feature = "runtime-airbyte")]
    runtimes.push("airbyte".to_string());

    Json(HealthResponse {
        status: "ok".to_string(),
        version: crate::VERSION.to_string(),
        runtimes,
    })
}
