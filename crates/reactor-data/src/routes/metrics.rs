//! Metrics endpoint.
//!
//! Provides basic server metrics in JSON format.
//! Full Prometheus metrics support planned for v0.2.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

/// Basic metrics response.
#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    /// Server version.
    pub version: &'static str,
    /// Uptime would go here in a full implementation.
    pub status: &'static str,
}

/// GET /data/v1/metrics
///
/// Returns basic server metrics. Gated by REACTOR_DATA_METRICS=1.
pub async fn metrics() -> impl IntoResponse {
    let response = MetricsResponse {
        version: crate::VERSION,
        status: "ok",
    };

    (StatusCode::OK, Json(response))
}
