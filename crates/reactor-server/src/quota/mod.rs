//! Per-tenant quota enforcement middleware (Phase 4).
//!
//! This module implements rate limiting and quota enforcement for multi-tenant
//! shared cluster deployments. Each tenant has limits on:
//!
//! - Requests per minute (token bucket)
//! - Concurrent function invocations
//! - Database connections (enforced at pooler level)
//! - Storage usage (GB)
//! - Bandwidth per month (GB)
//!
//! # Architecture
//!
//! ```text
//! Request → QuotaMiddleware → [rate_limit_check] → Handler
//!                                    ↓
//!                              QuotaService
//!                                    ↓
//!                           ┌───────────────────┐
//!                           │ Local Buckets     │  (per-node)
//!                           │ NATS Counters     │  (cross-node, optional)
//!                           └───────────────────┘
//! ```
//!
//! # Rate limiting strategy
//!
//! - **Requests/min**: Token bucket with replenishment via background tick.
//!   In shared cluster mode, counters are synced via NATS for approximate
//!   global enforcement.
//!
//! - **Concurrent functions**: Atomic counters per node. Acceptable for free
//!   tier; can be promoted to distributed counters if needed.
//!
//! - **DB connections**: Enforced at Supavisor via `ALTER ROLE ... CONNECTION LIMIT`.
//!
//! - **Storage GB**: Checked async on every upload. Project marked read_only on breach.
//!
//! - **Bandwidth GB/month**: Gateway-level metering via access logs, aggregated nightly.

mod bucket;
mod middleware;
mod service;

pub use bucket::TokenBucket;
pub use middleware::quota_middleware;
pub use service::{QuotaLimits, QuotaService, QuotaServiceConfig};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Quota exceeded error response.
#[derive(Debug, Serialize)]
pub struct QuotaExceededResponse {
    /// Error code.
    pub error: &'static str,
    /// Human-readable message.
    pub message: String,
    /// Quota that was exceeded.
    pub quota: &'static str,
    /// Current limit.
    pub limit: u32,
    /// Seconds until quota resets.
    pub retry_after_secs: u64,
}

impl IntoResponse for QuotaExceededResponse {
    fn into_response(self) -> Response {
        let retry_after = self.retry_after_secs.to_string();
        
        (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", retry_after)],
            Json(serde_json::json!({
                "ok": false,
                "error": {
                    "code": self.error,
                    "message": self.message,
                    "quota": self.quota,
                    "limit": self.limit,
                    "retry_after_secs": self.retry_after_secs,
                }
            })),
        )
            .into_response()
    }
}
