//! Metrics endpoint for Prometheus scraping.

use axum::{extract::State, http::StatusCode, response::IntoResponse};

use crate::state::StorageState;

/// GET /storage/v1/metrics
///
/// Returns Prometheus-format metrics.
pub async fn metrics(State(state): State<StorageState>) -> impl IntoResponse {
    if !state.config.metrics {
        return (StatusCode::NOT_FOUND, "Metrics disabled".to_string());
    }

    // Basic metrics in Prometheus format
    // In production, you'd use a metrics library like prometheus-client
    let mut output = String::new();

    output.push_str("# HELP reactor_storage_up Storage service is up\n");
    output.push_str("# TYPE reactor_storage_up gauge\n");
    output.push_str("reactor_storage_up 1\n");

    output.push_str("# HELP reactor_storage_info Storage service info\n");
    output.push_str("# TYPE reactor_storage_info gauge\n");
    output.push_str(&format!(
        "reactor_storage_info{{version=\"{}\"}} 1\n",
        crate::VERSION
    ));

    (StatusCode::OK, output)
}
