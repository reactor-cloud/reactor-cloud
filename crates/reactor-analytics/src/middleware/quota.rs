//! Quota and rate limiting middleware.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::AnalyticsConfig;
use crate::error::AnalyticsError;
use crate::store::AnalyticsStore;

/// Cached quota state for an organization.
#[derive(Debug, Clone)]
pub struct OrgQuotaState {
    /// Monthly event count (cached).
    pub event_count: u64,
    /// When the count was last refreshed.
    pub last_refresh: Instant,
    /// Quota limit.
    pub limit: u64,
}

/// Token bucket for per-key rate limiting.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Current number of tokens.
    pub tokens: f64,
    /// Last time tokens were updated.
    pub last_update: Instant,
    /// Max tokens (burst size).
    pub max_tokens: f64,
    /// Token refill rate per second.
    pub refill_rate: f64,
}

impl TokenBucket {
    /// Create a new token bucket.
    pub fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            last_update: Instant::now(),
            max_tokens,
            refill_rate,
        }
    }

    /// Try to consume a token. Returns true if successful.
    pub fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Try to consume multiple tokens. Returns true if successful.
    pub fn try_consume_n(&mut self, n: usize) -> bool {
        self.refill();
        let required = n as f64;
        if self.tokens >= required {
            self.tokens -= required;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_update = now;
    }
}

/// Quota manager with caching and rate limiting.
pub struct QuotaManager<S: AnalyticsStore> {
    store: Arc<S>,
    config: Arc<AnalyticsConfig>,
    /// Cached monthly quota states per org.
    org_quotas: RwLock<HashMap<Uuid, OrgQuotaState>>,
    /// Per-key rate limit buckets.
    key_buckets: RwLock<HashMap<Uuid, TokenBucket>>,
}

impl<S: AnalyticsStore> QuotaManager<S> {
    /// Create a new quota manager.
    pub fn new(store: Arc<S>, config: Arc<AnalyticsConfig>) -> Self {
        Self {
            store,
            config,
            org_quotas: RwLock::new(HashMap::new()),
            key_buckets: RwLock::new(HashMap::new()),
        }
    }

    /// Check if an org has quota available.
    /// Returns Ok(()) if quota is available, Err(AnalyticsError) if exceeded.
    pub async fn check_org_quota(&self, org_id: Uuid) -> Result<(), AnalyticsError> {
        let cache_ttl = Duration::from_secs(self.config.quota_cache_ttl_secs);

        // Check cache first
        {
            let quotas = self.org_quotas.read().await;
            if let Some(state) = quotas.get(&org_id) {
                if state.last_refresh.elapsed() < cache_ttl {
                    if state.event_count >= state.limit {
                        return Err(AnalyticsError::QuotaExceeded {
                            org_id,
                            limit: state.limit,
                        });
                    }
                    return Ok(());
                }
            }
        }

        // Refresh from store
        let count = self.store.get_org_monthly_event_count(org_id).await?;
        let limit = self.config.quota_per_org_monthly;

        {
            let mut quotas = self.org_quotas.write().await;
            quotas.insert(
                org_id,
                OrgQuotaState {
                    event_count: count,
                    last_refresh: Instant::now(),
                    limit,
                },
            );
        }

        if count >= limit {
            return Err(AnalyticsError::QuotaExceeded { org_id, limit });
        }

        Ok(())
    }

    /// Increment the cached event count for an org.
    pub async fn increment_org_count(&self, org_id: Uuid, count: u64) {
        let mut quotas = self.org_quotas.write().await;
        if let Some(state) = quotas.get_mut(&org_id) {
            state.event_count = state.event_count.saturating_add(count);
        }
    }

    /// Check rate limit for a key. Returns Ok(()) if allowed.
    pub async fn check_key_rate_limit(&self, key_id: Uuid) -> Result<(), AnalyticsError> {
        let mut buckets = self.key_buckets.write().await;

        let bucket = buckets.entry(key_id).or_insert_with(|| {
            TokenBucket::new(
                self.config.rate_limit_burst as f64,
                self.config.rate_limit_per_second as f64,
            )
        });

        if bucket.try_consume() {
            Ok(())
        } else {
            Err(AnalyticsError::RateLimited)
        }
    }

    /// Check rate limit for a batch of events.
    pub async fn check_key_rate_limit_batch(
        &self,
        key_id: Uuid,
        count: usize,
    ) -> Result<(), AnalyticsError> {
        let mut buckets = self.key_buckets.write().await;

        let bucket = buckets.entry(key_id).or_insert_with(|| {
            TokenBucket::new(
                self.config.rate_limit_burst as f64,
                self.config.rate_limit_per_second as f64,
            )
        });

        if bucket.try_consume_n(count) {
            Ok(())
        } else {
            Err(AnalyticsError::RateLimited)
        }
    }

    /// Clean up old entries (call periodically).
    pub async fn cleanup(&self) {
        let stale_threshold = Duration::from_secs(3600); // 1 hour

        // Clean org quotas
        {
            let mut quotas = self.org_quotas.write().await;
            quotas.retain(|_, v| v.last_refresh.elapsed() < stale_threshold);
        }

        // Clean key buckets
        {
            let mut buckets = self.key_buckets.write().await;
            buckets.retain(|_, v| v.last_update.elapsed() < stale_threshold);
        }
    }
}

/// Deterministic sampling based on event hash.
pub struct Sampler;

impl Sampler {
    /// Determine if an event should be sampled in, based on anonymous_id and sample_rate.
    ///
    /// sample_rate is a value between 0.0 and 1.0.
    /// Returns true if the event should be kept.
    pub fn should_sample(anonymous_id: &str, sample_rate: f64) -> bool {
        if sample_rate >= 1.0 {
            return true;
        }
        if sample_rate <= 0.0 {
            return false;
        }

        // Use a hash of the anonymous_id for deterministic sampling
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        anonymous_id.hash(&mut hasher);
        let hash = hasher.finish();

        // Map hash to 0.0..1.0 range
        let normalized = (hash as f64) / (u64::MAX as f64);
        normalized < sample_rate
    }

    /// Sample a batch of events based on anonymous_id and sample_rate.
    /// Returns indices of events to keep.
    pub fn sample_batch(
        events: &[crate::ingest::IngestEvent],
        sample_rate: f64,
    ) -> Vec<usize> {
        if sample_rate >= 1.0 {
            return (0..events.len()).collect();
        }
        if sample_rate <= 0.0 {
            return Vec::new();
        }

        events
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.anonymous_id
                    .as_ref()
                    .map(|id| Self::should_sample(id, sample_rate))
                    .unwrap_or(true)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(10.0, 1.0);

        // Should have 10 tokens initially
        assert!(bucket.try_consume_n(5));
        assert!(bucket.try_consume_n(5));

        // Should be empty now
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_deterministic_sampling() {
        // Same ID should always produce same result
        let result1 = Sampler::should_sample("test-id-123", 0.5);
        let result2 = Sampler::should_sample("test-id-123", 0.5);
        assert_eq!(result1, result2);

        // 100% sample rate should always include
        assert!(Sampler::should_sample("any-id", 1.0));

        // 0% sample rate should never include
        assert!(!Sampler::should_sample("any-id", 0.0));
    }
}
