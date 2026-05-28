//! Prometheus metrics endpoint.

use axum::{extract::State, http::StatusCode, response::IntoResponse};

use crate::state::JobsState;

/// Metrics endpoint (gated by REACTOR_JOBS_METRICS=1).
///
/// GET /jobs/v1/metrics
pub async fn metrics(State(state): State<JobsState>) -> impl IntoResponse {
    if !state.config.metrics {
        return (StatusCode::NOT_FOUND, "Metrics disabled".to_string());
    }

    // In production, this would use prometheus crate to collect metrics
    // For now, return a basic metrics output
    let output = format!(
        r#"# HELP jobs_runs_total Total number of job runs
# TYPE jobs_runs_total counter
jobs_runs_total{{status="pending"}} 0
jobs_runs_total{{status="running"}} 0
jobs_runs_total{{status="succeeded"}} 0
jobs_runs_total{{status="failed"}} 0

# HELP jobs_run_duration_seconds Duration of job runs
# TYPE jobs_run_duration_seconds histogram
jobs_run_duration_seconds_bucket{{le="0.1"}} 0
jobs_run_duration_seconds_bucket{{le="0.5"}} 0
jobs_run_duration_seconds_bucket{{le="1"}} 0
jobs_run_duration_seconds_bucket{{le="5"}} 0
jobs_run_duration_seconds_bucket{{le="10"}} 0
jobs_run_duration_seconds_bucket{{le="+Inf"}} 0
jobs_run_duration_seconds_sum 0
jobs_run_duration_seconds_count 0

# HELP jobs_steps_total Total number of steps executed
# TYPE jobs_steps_total counter
jobs_steps_total{{status="completed"}} 0
jobs_steps_total{{status="failed"}} 0

# HELP jobs_retries_total Total number of retries
# TYPE jobs_retries_total counter
jobs_retries_total 0

# HELP jobs_dlq_total Total number of runs moved to DLQ
# TYPE jobs_dlq_total counter
jobs_dlq_total 0

# HELP jobs_concurrency_rejected_total Total number of runs rejected due to concurrency
# TYPE jobs_concurrency_rejected_total counter
jobs_concurrency_rejected_total 0

# HELP jobs_queue_depth Number of items in queue
# TYPE jobs_queue_depth gauge
jobs_queue_depth 0

# HELP jobs_sleeping_runs Number of sleeping runs
# TYPE jobs_sleeping_runs gauge
jobs_sleeping_runs 0

# HELP jobs_info Information about the jobs server
# TYPE jobs_info gauge
jobs_info{{version="{}"}} 1
"#,
        crate::VERSION
    );

    (StatusCode::OK, output)
}
