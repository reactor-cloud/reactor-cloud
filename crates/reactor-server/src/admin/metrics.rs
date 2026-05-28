//! Prometheus metrics endpoint.

use axum::{http::StatusCode, response::IntoResponse};

/// Metrics handler.
///
/// Returns Prometheus-format metrics from all capabilities.
pub async fn metrics_handler() -> impl IntoResponse {
    // In a full implementation, this would aggregate metrics from
    // the unified metrics registry. For now, return empty metrics.
    let body = "# HELP reactor_server_info Server information\n\
                # TYPE reactor_server_info gauge\n\
                reactor_server_info{version=\"0.1.0\"} 1\n";

    (
        StatusCode::OK,
        [("content-type", "text/plain; charset=utf-8")],
        body,
    )
}
