//! Quota service for managing per-tenant rate limits.

use dashmap::DashMap;
use reactor_core::ProjectId;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::bucket::TokenBucket;

/// Quota limits for a tenant.
#[derive(Debug, Clone)]
pub struct QuotaLimits {
    /// Requests per minute.
    pub requests_per_minute: u32,

    /// Maximum concurrent function invocations.
    pub concurrent_functions: u32,

    /// Maximum database connections (enforced at pooler).
    pub db_connections: u32,

    /// Storage quota in bytes.
    pub storage_bytes: u64,

    /// Bandwidth quota per month in bytes.
    pub bandwidth_bytes_per_month: u64,
}

impl QuotaLimits {
    /// Create quota limits for free tier.
    pub fn free_tier() -> Self {
        Self {
            requests_per_minute: 1000,
            concurrent_functions: 10,
            db_connections: 5,
            storage_bytes: 1_073_741_824, // 1 GB
            bandwidth_bytes_per_month: 5_368_709_120, // 5 GB
        }
    }

    /// Create unlimited quotas (dedicated tier).
    pub fn unlimited() -> Self {
        Self {
            requests_per_minute: u32::MAX,
            concurrent_functions: u32::MAX,
            db_connections: 100,
            storage_bytes: u64::MAX,
            bandwidth_bytes_per_month: u64::MAX,
        }
    }

    /// Check if these limits are effectively unlimited.
    pub fn is_unlimited(&self) -> bool {
        self.requests_per_minute == u32::MAX
    }
}

impl Default for QuotaLimits {
    fn default() -> Self {
        Self::free_tier()
    }
}

/// Per-tenant quota state.
struct TenantQuotaState {
    /// Request rate limiter.
    request_bucket: TokenBucket,

    /// Current concurrent function count.
    concurrent_functions: AtomicI32,

    /// Current storage usage in bytes.
    storage_used_bytes: AtomicU64,

    /// Current month's bandwidth usage in bytes.
    bandwidth_used_bytes: AtomicU64,

    /// The limits for this tenant.
    limits: QuotaLimits,
}

impl TenantQuotaState {
    fn new(limits: QuotaLimits) -> Self {
        Self {
            request_bucket: TokenBucket::for_requests_per_minute(limits.requests_per_minute),
            concurrent_functions: AtomicI32::new(0),
            storage_used_bytes: AtomicU64::new(0),
            bandwidth_used_bytes: AtomicU64::new(0),
            limits,
        }
    }
}

/// Configuration for the quota service.
#[derive(Debug, Clone)]
pub struct QuotaServiceConfig {
    /// Default limits for free tier.
    pub free_tier_limits: QuotaLimits,

    /// Default limits for dedicated tier (None = unlimited).
    pub dedicated_tier_limits: Option<QuotaLimits>,

    /// How long to cache tenant quota state.
    pub cache_ttl: Duration,
}

impl Default for QuotaServiceConfig {
    fn default() -> Self {
        Self {
            free_tier_limits: QuotaLimits::free_tier(),
            dedicated_tier_limits: None,
            cache_ttl: Duration::from_secs(300),
        }
    }
}

/// Quota check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaCheckResult {
    /// Request is allowed.
    Allowed,
    /// Request is rate-limited (requests per minute exceeded).
    RateLimited { retry_after_secs: u64 },
    /// Too many concurrent functions.
    ConcurrentFunctionsExceeded,
    /// Storage quota exceeded.
    StorageExceeded,
    /// Bandwidth quota exceeded.
    BandwidthExceeded,
}

impl QuotaCheckResult {
    /// Check if the result is allowed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Quota service for managing per-tenant rate limits.
pub struct QuotaService {
    /// Per-tenant quota state, keyed by ProjectId.
    tenants: DashMap<ProjectId, Arc<TenantQuotaState>>,

    /// Configuration.
    config: QuotaServiceConfig,
}

impl QuotaService {
    /// Create a new quota service.
    pub fn new(config: QuotaServiceConfig) -> Self {
        Self {
            tenants: DashMap::new(),
            config,
        }
    }

    /// Get the limits for a tenant.
    pub fn limits_for(&self, project_id: &ProjectId, is_dedicated: bool) -> QuotaLimits {
        if is_dedicated {
            self.config
                .dedicated_tier_limits
                .clone()
                .unwrap_or_else(QuotaLimits::unlimited)
        } else {
            self.config.free_tier_limits.clone()
        }
    }

    /// Get or create quota state for a tenant.
    fn get_or_create(&self, project_id: &ProjectId, limits: &QuotaLimits) -> Arc<TenantQuotaState> {
        if let Some(state) = self.tenants.get(project_id) {
            return state.clone();
        }

        let state = Arc::new(TenantQuotaState::new(limits.clone()));
        self.tenants.insert(*project_id, state.clone());
        state
    }

    /// Check if a request is allowed (rate limit check).
    pub fn check_request(&self, project_id: &ProjectId, is_dedicated: bool) -> QuotaCheckResult {
        let limits = self.limits_for(project_id, is_dedicated);

        // Skip check for unlimited
        if limits.is_unlimited() {
            return QuotaCheckResult::Allowed;
        }

        let state = self.get_or_create(project_id, &limits);

        if state.request_bucket.try_consume() {
            QuotaCheckResult::Allowed
        } else {
            let retry_after = state.request_bucket.retry_after();
            debug!(
                project_id = %project_id,
                retry_after_secs = retry_after,
                "rate limit exceeded"
            );
            QuotaCheckResult::RateLimited {
                retry_after_secs: retry_after,
            }
        }
    }

    /// Try to acquire a function execution slot.
    ///
    /// Returns `Some(guard)` if successful, `None` if limit exceeded.
    pub fn try_acquire_function(&self, project_id: &ProjectId, is_dedicated: bool) -> Option<FunctionGuard> {
        let limits = self.limits_for(project_id, is_dedicated);

        // Skip check for unlimited
        if limits.concurrent_functions == u32::MAX {
            return Some(FunctionGuard {
                project_id: *project_id,
                state: None,
            });
        }

        let state = self.get_or_create(project_id, &limits);

        // Try to increment
        loop {
            let current = state.concurrent_functions.load(Ordering::Acquire);

            if current >= limits.concurrent_functions as i32 {
                debug!(
                    project_id = %project_id,
                    current = current,
                    limit = limits.concurrent_functions,
                    "concurrent functions limit exceeded"
                );
                return None;
            }

            if state
                .concurrent_functions
                .compare_exchange_weak(current, current + 1, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                return Some(FunctionGuard {
                    project_id: *project_id,
                    state: Some(state),
                });
            }
        }
    }

    /// Check storage quota.
    ///
    /// Returns `true` if upload is allowed, `false` if quota exceeded.
    pub fn check_storage(
        &self,
        project_id: &ProjectId,
        is_dedicated: bool,
        additional_bytes: u64,
    ) -> QuotaCheckResult {
        let limits = self.limits_for(project_id, is_dedicated);

        if limits.storage_bytes == u64::MAX {
            return QuotaCheckResult::Allowed;
        }

        let state = self.get_or_create(project_id, &limits);
        let current = state.storage_used_bytes.load(Ordering::Relaxed);

        if current + additional_bytes > limits.storage_bytes {
            QuotaCheckResult::StorageExceeded
        } else {
            QuotaCheckResult::Allowed
        }
    }

    /// Record storage usage change.
    pub fn record_storage_change(&self, project_id: &ProjectId, delta_bytes: i64) {
        if let Some(state) = self.tenants.get(project_id) {
            if delta_bytes >= 0 {
                state
                    .storage_used_bytes
                    .fetch_add(delta_bytes as u64, Ordering::Relaxed);
            } else {
                let abs_delta = (-delta_bytes) as u64;
                state
                    .storage_used_bytes
                    .fetch_sub(abs_delta.min(state.storage_used_bytes.load(Ordering::Relaxed)), Ordering::Relaxed);
            }
        }
    }

    /// Check bandwidth quota.
    pub fn check_bandwidth(
        &self,
        project_id: &ProjectId,
        is_dedicated: bool,
        bytes: u64,
    ) -> QuotaCheckResult {
        let limits = self.limits_for(project_id, is_dedicated);

        if limits.bandwidth_bytes_per_month == u64::MAX {
            return QuotaCheckResult::Allowed;
        }

        let state = self.get_or_create(project_id, &limits);
        let current = state.bandwidth_used_bytes.load(Ordering::Relaxed);

        if current + bytes > limits.bandwidth_bytes_per_month {
            QuotaCheckResult::BandwidthExceeded
        } else {
            QuotaCheckResult::Allowed
        }
    }

    /// Record bandwidth usage.
    pub fn record_bandwidth(&self, project_id: &ProjectId, bytes: u64) {
        if let Some(state) = self.tenants.get(project_id) {
            state.bandwidth_used_bytes.fetch_add(bytes, Ordering::Relaxed);
        }
    }

    /// Reset monthly bandwidth counters (call at month start).
    pub fn reset_monthly_bandwidth(&self) {
        for entry in self.tenants.iter() {
            entry.bandwidth_used_bytes.store(0, Ordering::Relaxed);
        }
        info!("reset monthly bandwidth counters for {} tenants", self.tenants.len());
    }

    /// Clear quota state for a tenant.
    pub fn clear_tenant(&self, project_id: &ProjectId) {
        self.tenants.remove(project_id);
    }

    /// Get current stats for a tenant.
    pub fn stats(&self, project_id: &ProjectId) -> Option<TenantQuotaStats> {
        self.tenants.get(project_id).map(|state| TenantQuotaStats {
            available_requests: state.request_bucket.available(),
            concurrent_functions: state.concurrent_functions.load(Ordering::Relaxed),
            storage_used_bytes: state.storage_used_bytes.load(Ordering::Relaxed),
            bandwidth_used_bytes: state.bandwidth_used_bytes.load(Ordering::Relaxed),
            limits: state.limits.clone(),
        })
    }
}

/// Guard for a function execution slot.
///
/// When dropped, releases the slot.
pub struct FunctionGuard {
    project_id: ProjectId,
    state: Option<Arc<TenantQuotaState>>,
}

impl Drop for FunctionGuard {
    fn drop(&mut self) {
        if let Some(ref state) = self.state {
            state.concurrent_functions.fetch_sub(1, Ordering::Release);
        }
    }
}

/// Quota statistics for a tenant.
#[derive(Debug, Clone)]
pub struct TenantQuotaStats {
    /// Available request tokens.
    pub available_requests: u32,
    /// Current concurrent functions.
    pub concurrent_functions: i32,
    /// Current storage usage in bytes.
    pub storage_used_bytes: u64,
    /// Current month's bandwidth usage in bytes.
    pub bandwidth_used_bytes: u64,
    /// The limits for this tenant.
    pub limits: QuotaLimits,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_project_id() -> ProjectId {
        ProjectId::new()
    }

    #[test]
    fn test_quota_limits_defaults() {
        let limits = QuotaLimits::default();
        assert_eq!(limits.requests_per_minute, 1000);
        assert_eq!(limits.concurrent_functions, 10);
    }

    #[test]
    fn test_check_request_allowed() {
        let service = QuotaService::new(QuotaServiceConfig::default());
        let project_id = test_project_id();

        let result = service.check_request(&project_id, false);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_check_request_rate_limited() {
        let mut config = QuotaServiceConfig::default();
        config.free_tier_limits.requests_per_minute = 1;

        let service = QuotaService::new(config);
        let project_id = test_project_id();

        // First request allowed
        assert!(service.check_request(&project_id, false).is_allowed());

        // Second request should be rate-limited (bucket size is rpm/6 = 0, minimum 1)
        // Actually with rpm=1, burst=1, so first succeeds, second fails
        let result = service.check_request(&project_id, false);
        assert!(matches!(result, QuotaCheckResult::RateLimited { .. }));
    }

    #[test]
    fn test_function_guard() {
        let service = QuotaService::new(QuotaServiceConfig::default());
        let project_id = test_project_id();

        // Acquire function slot
        let guard = service.try_acquire_function(&project_id, false);
        assert!(guard.is_some());

        // Check stats
        let stats = service.stats(&project_id).unwrap();
        assert_eq!(stats.concurrent_functions, 1);

        // Drop guard
        drop(guard);

        // Check stats again
        let stats = service.stats(&project_id).unwrap();
        assert_eq!(stats.concurrent_functions, 0);
    }

    #[test]
    fn test_dedicated_unlimited() {
        let service = QuotaService::new(QuotaServiceConfig::default());
        let project_id = test_project_id();

        // Dedicated should always be allowed
        for _ in 0..1000 {
            assert!(service.check_request(&project_id, true).is_allowed());
        }
    }

    #[test]
    fn test_storage_check() {
        let mut config = QuotaServiceConfig::default();
        config.free_tier_limits.storage_bytes = 1000;

        let service = QuotaService::new(config);
        let project_id = test_project_id();

        // Should be allowed under limit
        assert!(service.check_storage(&project_id, false, 500).is_allowed());

        // Record usage
        service.record_storage_change(&project_id, 800);

        // Should exceed with additional
        let result = service.check_storage(&project_id, false, 300);
        assert_eq!(result, QuotaCheckResult::StorageExceeded);
    }
}
