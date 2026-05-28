//! Health check endpoint.

use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AnalyticsState;
use crate::store::AnalyticsStore;

/// Health response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Overall status.
    pub status: String,
    /// Store connectivity status.
    pub store: String,
    /// Version.
    pub version: String,
}

/// Health check endpoint.
#[utoipa::path(
    get,
    path = "/analytics/v1/health",
    tag = "analytics",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
        (status = 503, description = "Service is degraded", body = HealthResponse)
    )
)]
pub async fn health<S: AnalyticsStore>(
    State(state): State<AnalyticsState<S>>,
) -> (StatusCode, Json<HealthResponse>) {
    let store_status = match sqlx::query("SELECT 1")
        .execute(state.store.pool())
        .await
    {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            tracing::error!(error = %e, "health check database query failed");
            format!("error: {}", e)
        }
    };

    let is_healthy = store_status == "ok";

    let response = HealthResponse {
        status: if is_healthy {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        store: store_status,
        version: crate::VERSION.to_string(),
    };

    let status = if is_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(response))
}
