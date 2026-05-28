//! Prometheus metrics endpoint.
//!
//! Exposes function metrics in Prometheus text format when
//! `REACTOR_FUNCTIONS_METRICS=1` is set.

use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::state::FunctionsState;

/// Global metrics counters.
/// 
/// These are exposed via the /metrics endpoint when metrics are enabled.
#[derive(Debug, Default)]
pub struct FunctionMetrics {
    /// Total number of invocations.
    pub invocations_total: AtomicU64,
    /// Total successful invocations.
    pub invocations_success: AtomicU64,
    /// Total failed invocations.
    pub invocations_error: AtomicU64,
    /// Total timed out invocations.
    pub invocations_timeout: AtomicU64,
    /// Total cold starts.
    pub cold_starts_total: AtomicU64,
    /// Total request bytes.
    pub request_bytes_total: AtomicU64,
    /// Total response bytes.
    pub response_bytes_total: AtomicU64,
    /// Duration histogram bucket: <= 5ms.
    pub duration_bucket_5ms: AtomicU64,
    /// Duration histogram bucket: <= 10ms.
    pub duration_bucket_10ms: AtomicU64,
    /// Duration histogram bucket: <= 25ms.
    pub duration_bucket_25ms: AtomicU64,
    /// Duration histogram bucket: <= 50ms.
    pub duration_bucket_50ms: AtomicU64,
    /// Duration histogram bucket: <= 100ms.
    pub duration_bucket_100ms: AtomicU64,
    /// Duration histogram bucket: <= 250ms.
    pub duration_bucket_250ms: AtomicU64,
    /// Duration histogram bucket: <= 500ms.
    pub duration_bucket_500ms: AtomicU64,
    /// Duration histogram bucket: <= 1000ms.
    pub duration_bucket_1000ms: AtomicU64,
    /// Duration histogram bucket: <= 2500ms.
    pub duration_bucket_2500ms: AtomicU64,
    /// Duration histogram bucket: <= 5000ms.
    pub duration_bucket_5000ms: AtomicU64,
    /// Duration histogram bucket: <= 10000ms.
    pub duration_bucket_10000ms: AtomicU64,
    /// Duration histogram bucket: +Inf.
    pub duration_bucket_inf: AtomicU64,
    /// Duration histogram sum (milliseconds).
    pub duration_sum_ms: AtomicU64,
    /// Duration histogram count.
    pub duration_count: AtomicU64,
    /// Current concurrent invocations.
    pub concurrent_invocations: AtomicU64,
    /// Total WASM invocations.
    pub wasm_invocations: AtomicU64,
    /// Total Bun invocations.
    pub bun_invocations: AtomicU64,
    /// Total Lambda invocations.
    pub lambda_invocations: AtomicU64,
}

impl FunctionMetrics {
    /// Create a new metrics instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an invocation duration for histogram.
    pub fn record_duration(&self, duration_ms: u64) {
        self.duration_sum_ms.fetch_add(duration_ms, Ordering::Relaxed);
        self.duration_count.fetch_add(1, Ordering::Relaxed);

        // Increment all buckets >= duration
        if duration_ms <= 5 {
            self.duration_bucket_5ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 10 {
            self.duration_bucket_10ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 25 {
            self.duration_bucket_25ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 50 {
            self.duration_bucket_50ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 100 {
            self.duration_bucket_100ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 250 {
            self.duration_bucket_250ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 500 {
            self.duration_bucket_500ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 1000 {
            self.duration_bucket_1000ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 2500 {
            self.duration_bucket_2500ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 5000 {
            self.duration_bucket_5000ms.fetch_add(1, Ordering::Relaxed);
        }
        if duration_ms <= 10000 {
            self.duration_bucket_10000ms.fetch_add(1, Ordering::Relaxed);
        }
        self.duration_bucket_inf.fetch_add(1, Ordering::Relaxed);
    }
}

/// GET /metrics
///
/// Returns Prometheus-formatted metrics.
/// Only available when REACTOR_FUNCTIONS_METRICS=1.
pub async fn metrics_handler(
    State(state): State<FunctionsState>,
) -> impl IntoResponse {
    // Check if metrics are enabled
    if !state.config.metrics {
        return Response::builder()
            .status(404)
            .body("Metrics not enabled".to_string())
            .unwrap();
    }

    let metrics = &state.metrics;
    let mut output = String::new();

    // Invocation counters
    output.push_str("# HELP reactor_functions_invocations_total Total number of function invocations\n");
    output.push_str("# TYPE reactor_functions_invocations_total counter\n");
    output.push_str(&format!(
        "reactor_functions_invocations_total {}\n",
        metrics.invocations_total.load(Ordering::Relaxed)
    ));

    output.push_str("# HELP reactor_functions_invocations_success Total successful invocations\n");
    output.push_str("# TYPE reactor_functions_invocations_success counter\n");
    output.push_str(&format!(
        "reactor_functions_invocations_success {}\n",
        metrics.invocations_success.load(Ordering::Relaxed)
    ));

    output.push_str("# HELP reactor_functions_invocations_error Total failed invocations\n");
    output.push_str("# TYPE reactor_functions_invocations_error counter\n");
    output.push_str(&format!(
        "reactor_functions_invocations_error {}\n",
        metrics.invocations_error.load(Ordering::Relaxed)
    ));

    output.push_str("# HELP reactor_functions_invocations_timeout Total timed out invocations\n");
    output.push_str("# TYPE reactor_functions_invocations_timeout counter\n");
    output.push_str(&format!(
        "reactor_functions_invocations_timeout {}\n",
        metrics.invocations_timeout.load(Ordering::Relaxed)
    ));

    // Cold starts
    output.push_str("# HELP reactor_functions_cold_starts_total Total cold starts\n");
    output.push_str("# TYPE reactor_functions_cold_starts_total counter\n");
    output.push_str(&format!(
        "reactor_functions_cold_starts_total {}\n",
        metrics.cold_starts_total.load(Ordering::Relaxed)
    ));

    // Bytes
    output.push_str("# HELP reactor_functions_request_bytes_total Total request bytes\n");
    output.push_str("# TYPE reactor_functions_request_bytes_total counter\n");
    output.push_str(&format!(
        "reactor_functions_request_bytes_total {}\n",
        metrics.request_bytes_total.load(Ordering::Relaxed)
    ));

    output.push_str("# HELP reactor_functions_response_bytes_total Total response bytes\n");
    output.push_str("# TYPE reactor_functions_response_bytes_total counter\n");
    output.push_str(&format!(
        "reactor_functions_response_bytes_total {}\n",
        metrics.response_bytes_total.load(Ordering::Relaxed)
    ));

    // Duration histogram
    output.push_str("# HELP reactor_functions_duration_ms Invocation duration in milliseconds\n");
    output.push_str("# TYPE reactor_functions_duration_ms histogram\n");
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"5\"}} {}\n",
        metrics.duration_bucket_5ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"10\"}} {}\n",
        metrics.duration_bucket_10ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"25\"}} {}\n",
        metrics.duration_bucket_25ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"50\"}} {}\n",
        metrics.duration_bucket_50ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"100\"}} {}\n",
        metrics.duration_bucket_100ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"250\"}} {}\n",
        metrics.duration_bucket_250ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"500\"}} {}\n",
        metrics.duration_bucket_500ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"1000\"}} {}\n",
        metrics.duration_bucket_1000ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"2500\"}} {}\n",
        metrics.duration_bucket_2500ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"5000\"}} {}\n",
        metrics.duration_bucket_5000ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"10000\"}} {}\n",
        metrics.duration_bucket_10000ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_bucket{{le=\"+Inf\"}} {}\n",
        metrics.duration_bucket_inf.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_sum {}\n",
        metrics.duration_sum_ms.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_duration_ms_count {}\n",
        metrics.duration_count.load(Ordering::Relaxed)
    ));

    // Concurrency gauge
    output.push_str("# HELP reactor_functions_concurrent_invocations Current concurrent invocations\n");
    output.push_str("# TYPE reactor_functions_concurrent_invocations gauge\n");
    output.push_str(&format!(
        "reactor_functions_concurrent_invocations {}\n",
        metrics.concurrent_invocations.load(Ordering::Relaxed)
    ));

    // Runtime-specific counters
    output.push_str("# HELP reactor_functions_invocations_by_runtime Invocations by runtime type\n");
    output.push_str("# TYPE reactor_functions_invocations_by_runtime counter\n");
    output.push_str(&format!(
        "reactor_functions_invocations_by_runtime{{runtime=\"wasm\"}} {}\n",
        metrics.wasm_invocations.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_invocations_by_runtime{{runtime=\"bun\"}} {}\n",
        metrics.bun_invocations.load(Ordering::Relaxed)
    ));
    output.push_str(&format!(
        "reactor_functions_invocations_by_runtime{{runtime=\"lambda\"}} {}\n",
        metrics.lambda_invocations.load(Ordering::Relaxed)
    ));

    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(output)
        .unwrap()
}
