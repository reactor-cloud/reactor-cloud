//! Prometheus metrics endpoint.

use axum::response::IntoResponse;
use std::sync::atomic::{AtomicU64, Ordering};

/// Site metrics.
pub struct SiteMetrics {
    requests_total: AtomicU64,
    static_hits: AtomicU64,
    function_dispatches: AtomicU64,
    isr_hits: AtomicU64,
    isr_misses: AtomicU64,
    isr_stale: AtomicU64,
    policy_denied: AtomicU64,
}

impl SiteMetrics {
    /// Create new metrics.
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            static_hits: AtomicU64::new(0),
            function_dispatches: AtomicU64::new(0),
            isr_hits: AtomicU64::new(0),
            isr_misses: AtomicU64::new(0),
            isr_stale: AtomicU64::new(0),
            policy_denied: AtomicU64::new(0),
        }
    }

    /// Increment requests total.
    pub fn inc_requests(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment static hits.
    pub fn inc_static_hits(&self) {
        self.static_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment function dispatches.
    pub fn inc_function_dispatches(&self) {
        self.function_dispatches.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment ISR hits.
    pub fn inc_isr_hits(&self) {
        self.isr_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment ISR misses.
    pub fn inc_isr_misses(&self) {
        self.isr_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment ISR stale.
    pub fn inc_isr_stale(&self) {
        self.isr_stale.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment policy denied.
    pub fn inc_policy_denied(&self) {
        self.policy_denied.fetch_add(1, Ordering::Relaxed);
    }

    /// Export metrics in Prometheus format.
    pub fn export(&self) -> String {
        let mut output = String::new();

        output.push_str("# HELP sites_requests_total Total number of requests\n");
        output.push_str("# TYPE sites_requests_total counter\n");
        output.push_str(&format!(
            "sites_requests_total {}\n",
            self.requests_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP sites_static_hits_total Total number of static file hits\n");
        output.push_str("# TYPE sites_static_hits_total counter\n");
        output.push_str(&format!(
            "sites_static_hits_total {}\n",
            self.static_hits.load(Ordering::Relaxed)
        ));

        output.push_str(
            "# HELP sites_function_dispatches_total Total number of function dispatches\n",
        );
        output.push_str("# TYPE sites_function_dispatches_total counter\n");
        output.push_str(&format!(
            "sites_function_dispatches_total {}\n",
            self.function_dispatches.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP sites_isr_hits_total Total ISR cache hits\n");
        output.push_str("# TYPE sites_isr_hits_total counter\n");
        output.push_str(&format!(
            "sites_isr_hits_total {}\n",
            self.isr_hits.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP sites_isr_misses_total Total ISR cache misses\n");
        output.push_str("# TYPE sites_isr_misses_total counter\n");
        output.push_str(&format!(
            "sites_isr_misses_total {}\n",
            self.isr_misses.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP sites_isr_stale_total Total ISR stale responses\n");
        output.push_str("# TYPE sites_isr_stale_total counter\n");
        output.push_str(&format!(
            "sites_isr_stale_total {}\n",
            self.isr_stale.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP sites_policy_denied_total Total policy denied responses\n");
        output.push_str("# TYPE sites_policy_denied_total counter\n");
        output.push_str(&format!(
            "sites_policy_denied_total {}\n",
            self.policy_denied.load(Ordering::Relaxed)
        ));

        output
    }
}

impl Default for SiteMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics handler.
pub async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<crate::SitesState>,
) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        state.metrics.export(),
    )
}
