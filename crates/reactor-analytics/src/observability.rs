//! Observability utilities for analytics.

use metrics::{counter, gauge, histogram};
use std::time::Duration;

/// Metric names for analytics.
pub mod metric_names {
    // Ingestion metrics
    pub const EVENTS_RECEIVED: &str = "analytics_events_received_total";
    pub const EVENTS_ACCEPTED: &str = "analytics_events_accepted_total";
    pub const EVENTS_REJECTED: &str = "analytics_events_rejected_total";
    pub const EVENTS_TOMBSTONED: &str = "analytics_events_tombstoned_total";

    // Batch metrics
    pub const BATCH_FLUSH: &str = "analytics_batch_flush_total";
    pub const BATCH_SIZE: &str = "analytics_batch_size";
    pub const BATCH_LATENCY: &str = "analytics_batch_latency_seconds";

    // Query metrics
    pub const QUERIES: &str = "analytics_queries_total";
    pub const QUERY_LATENCY: &str = "analytics_query_latency_seconds";
    pub const QUERY_ROWS_SCANNED: &str = "analytics_query_rows_scanned";

    // Quota metrics
    pub const QUOTA_EVENTS: &str = "analytics_org_monthly_events";
    pub const QUOTA_EXCEEDED: &str = "analytics_quota_exceeded_total";
    pub const RATE_LIMITED: &str = "analytics_rate_limited_total";

    // Consent metrics
    pub const CONSENT_OPT_OUT: &str = "analytics_consent_opt_out_total";
    pub const CONSENT_OPT_IN: &str = "analytics_consent_opt_in_total";

    // Erasure metrics
    pub const ERASURES: &str = "analytics_erasures_total";
    pub const ERASURE_ROWS: &str = "analytics_erasure_rows_deleted_total";

    // Backpressure
    pub const BACKPRESSURE: &str = "analytics_backpressure_total";
}

/// Record an event received.
pub fn record_event_received(project_id: &str) {
    counter!(metric_names::EVENTS_RECEIVED, "project_id" => project_id.to_string())
        .increment(1);
}

/// Record events accepted.
pub fn record_events_accepted(project_id: &str, count: u64) {
    counter!(metric_names::EVENTS_ACCEPTED, "project_id" => project_id.to_string())
        .increment(count);
}

/// Record events rejected.
pub fn record_events_rejected(project_id: &str, reason: &str, count: u64) {
    counter!(
        metric_names::EVENTS_REJECTED,
        "project_id" => project_id.to_string(),
        "reason" => reason.to_string()
    )
    .increment(count);
}

/// Record events tombstoned (dropped due to opt-out).
pub fn record_events_tombstoned(project_id: &str, count: u64) {
    counter!(metric_names::EVENTS_TOMBSTONED, "project_id" => project_id.to_string())
        .increment(count);
}

/// Record a batch flush.
pub fn record_batch_flush(size: usize, latency: Duration) {
    counter!(metric_names::BATCH_FLUSH).increment(1);
    histogram!(metric_names::BATCH_SIZE).record(size as f64);
    histogram!(metric_names::BATCH_LATENCY).record(latency.as_secs_f64());
}

/// Record a query execution.
pub fn record_query(
    project_id: &str,
    kind: &str,
    latency: Duration,
    rows_scanned: u64,
    success: bool,
) {
    let status = if success { "ok" } else { "error" };
    counter!(
        metric_names::QUERIES,
        "project_id" => project_id.to_string(),
        "kind" => kind.to_string(),
        "status" => status.to_string()
    )
    .increment(1);

    histogram!(
        metric_names::QUERY_LATENCY,
        "kind" => kind.to_string()
    )
    .record(latency.as_secs_f64());

    histogram!(metric_names::QUERY_ROWS_SCANNED, "kind" => kind.to_string())
        .record(rows_scanned as f64);
}

/// Set the monthly event count for an org (for billing gauges).
pub fn set_org_monthly_events(org_id: &str, count: u64) {
    gauge!(metric_names::QUOTA_EVENTS, "org_id" => org_id.to_string())
        .set(count as f64);
}

/// Record quota exceeded.
pub fn record_quota_exceeded(org_id: &str) {
    counter!(metric_names::QUOTA_EXCEEDED, "org_id" => org_id.to_string())
        .increment(1);
}

/// Record rate limited.
pub fn record_rate_limited(key_id: &str) {
    counter!(metric_names::RATE_LIMITED, "key_id" => key_id.to_string())
        .increment(1);
}

/// Record consent opt-out.
pub fn record_consent_opt_out(project_id: &str) {
    counter!(metric_names::CONSENT_OPT_OUT, "project_id" => project_id.to_string())
        .increment(1);
}

/// Record consent opt-in.
pub fn record_consent_opt_in(project_id: &str) {
    counter!(metric_names::CONSENT_OPT_IN, "project_id" => project_id.to_string())
        .increment(1);
}

/// Record an erasure.
pub fn record_erasure(project_id: &str, subject_kind: &str, rows_deleted: u64) {
    counter!(
        metric_names::ERASURES,
        "project_id" => project_id.to_string(),
        "subject_kind" => subject_kind.to_string()
    )
    .increment(1);

    counter!(
        metric_names::ERASURE_ROWS,
        "project_id" => project_id.to_string(),
        "subject_kind" => subject_kind.to_string()
    )
    .increment(rows_deleted);
}

/// Record backpressure event.
pub fn record_backpressure(project_id: &str) {
    counter!(metric_names::BACKPRESSURE, "project_id" => project_id.to_string())
        .increment(1);
}

/// Standard tracing span fields for analytics requests.
#[derive(Debug, Clone)]
pub struct SpanFields {
    pub request_id: String,
    pub project_id: String,
    pub org_id: String,
    pub event_type: Option<String>,
    pub anonymous_id: Option<String>,
    pub user_id: Option<String>,
}

impl SpanFields {
    /// Create span fields from context.
    pub fn from_ctx(ctx: &crate::state::AnalyticsCtx) -> Self {
        Self {
            request_id: ctx.request_id.to_string(),
            project_id: ctx.project_id.to_string(),
            org_id: ctx.org_id.to_string(),
            event_type: None,
            anonymous_id: None,
            user_id: None,
        }
    }
}
