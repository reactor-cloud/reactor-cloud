//! Health check endpoint.

use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AiState;

/// Health check response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Service status.
    pub status: String,
    /// Service version.
    pub version: String,
    /// Available providers.
    pub providers: Vec<String>,
}

/// Health check endpoint.
#[utoipa::path(
    get,
    path = "/ai/v1/health",
    tag = "ai",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
    )
)]
pub async fn health(State(state): State<AiState>) -> (StatusCode, Json<HealthResponse>) {
    let providers = state
        .available_providers()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "healthy".to_string(),
            version: crate::VERSION.to_string(),
            providers,
        }),
    )
}
