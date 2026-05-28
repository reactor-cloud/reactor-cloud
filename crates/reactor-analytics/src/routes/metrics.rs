//! Prometheus metrics endpoint.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use metrics_exporter_prometheus::PrometheusHandle;

use crate::state::AnalyticsState;
use crate::store::AnalyticsStore;

/// Get the Prometheus metrics.
///
/// GET /analytics/v1/metrics
pub async fn metrics<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
) -> impl IntoResponse {
    // Check if metrics are enabled
    if !state.config.metrics {
        return (
            StatusCode::NOT_FOUND,
            "metrics endpoint disabled".to_string(),
        );
    }

    // Get the Prometheus handle from the recorder
    // In a real implementation, this would be injected via state
    // For now, just return a placeholder
    let output = String::from(
        "# HELP analytics_events_received_total Total events received\n\
         # TYPE analytics_events_received_total counter\n\
         # HELP analytics_events_accepted_total Total events accepted\n\
         # TYPE analytics_events_accepted_total counter\n\
         # HELP analytics_batch_flush_total Total batch flushes\n\
         # TYPE analytics_batch_flush_total counter\n\
         # HELP analytics_queries_total Total queries executed\n\
         # TYPE analytics_queries_total counter\n\
         # HELP analytics_org_monthly_events Monthly event count per org\n\
         # TYPE analytics_org_monthly_events gauge\n",
    );

    (StatusCode::OK, output)
}

/// Metrics endpoint handler with injected prometheus handle.
pub async fn metrics_with_handle(handle: PrometheusHandle) -> impl IntoResponse {
    let output = handle.render();
    (StatusCode::OK, output)
}
