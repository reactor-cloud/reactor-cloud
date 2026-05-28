//! Token bucket rate limiter.
//!
//! A simple token bucket implementation for rate limiting requests.
//! Tokens are replenished at a fixed rate and consumed on each request.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Token bucket rate limiter.
///
/// Each bucket has a maximum capacity and a refill rate. Tokens are
/// consumed on each request; if no tokens are available, the request
/// is rate-limited.
#[derive(Debug)]
pub struct TokenBucket {
    /// Current token count (scaled by 1000 for sub-token precision).
    tokens: AtomicU64,

    /// Maximum token count (scaled by 1000).
    capacity: u64,

    /// Tokens added per second (scaled by 1000).
    refill_rate: u64,

    /// Last refill timestamp (epoch millis).
    last_refill: AtomicU64,
}

impl TokenBucket {
    /// Create a new token bucket.
    ///
    /// # Arguments
    /// * `capacity` - Maximum tokens in the bucket
    /// * `refill_rate_per_sec` - Tokens added per second
    pub fn new(capacity: u32, refill_rate_per_sec: f64) -> Self {
        let capacity_scaled = (capacity as u64) * 1000;
        let refill_rate_scaled = (refill_rate_per_sec * 1000.0) as u64;

        Self {
            tokens: AtomicU64::new(capacity_scaled),
            capacity: capacity_scaled,
            refill_rate: refill_rate_scaled,
            last_refill: AtomicU64::new(Self::now_millis()),
        }
    }

    /// Create a bucket for requests-per-minute limiting.
    ///
    /// Capacity is set to allow bursts of up to `rpm / 6` (10 seconds worth),
    /// and refill rate is `rpm / 60` per second.
    pub fn for_requests_per_minute(rpm: u32) -> Self {
        let burst = rpm / 6; // Allow 10 seconds of burst
        let refill_rate = rpm as f64 / 60.0;
        Self::new(burst.max(1), refill_rate)
    }

    /// Try to consume one token.
    ///
    /// Returns `true` if a token was consumed, `false` if rate-limited.
    pub fn try_consume(&self) -> bool {
        self.try_consume_n(1)
    }

    /// Try to consume N tokens.
    ///
    /// Returns `true` if all tokens were consumed, `false` if rate-limited.
    pub fn try_consume_n(&self, n: u32) -> bool {
        let cost = (n as u64) * 1000;

        // Refill tokens based on elapsed time
        self.refill();

        // Try to consume
        loop {
            let current = self.tokens.load(Ordering::Acquire);

            if current < cost {
                return false;
            }

            let new = current - cost;

            if self
                .tokens
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }

    /// Get the current token count.
    pub fn available(&self) -> u32 {
        self.refill();
        (self.tokens.load(Ordering::Relaxed) / 1000) as u32
    }

    /// Get seconds until the bucket is fully refilled.
    pub fn time_to_full(&self) -> u64 {
        let current = self.tokens.load(Ordering::Relaxed);
        if current >= self.capacity {
            return 0;
        }

        let needed = self.capacity - current;
        let secs = needed / self.refill_rate;
        secs.max(1)
    }

    /// Get seconds until at least one token is available.
    pub fn retry_after(&self) -> u64 {
        let current = self.tokens.load(Ordering::Relaxed);
        if current >= 1000 {
            return 0;
        }

        let needed = 1000 - current;
        let secs = (needed + self.refill_rate - 1) / self.refill_rate;
        secs.max(1)
    }

    /// Refill tokens based on elapsed time.
    fn refill(&self) {
        let now = Self::now_millis();
        let last = self.last_refill.load(Ordering::Relaxed);

        if now <= last {
            return;
        }

        let elapsed_ms = now - last;
        let tokens_to_add = (elapsed_ms * self.refill_rate) / 1000;

        if tokens_to_add == 0 {
            return;
        }

        // Update last_refill timestamp
        if self
            .last_refill
            .compare_exchange(last, now, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            // Another thread updated it; they'll handle the refill
            return;
        }

        // Add tokens
        loop {
            let current = self.tokens.load(Ordering::Acquire);
            let new = (current + tokens_to_add).min(self.capacity);

            if self
                .tokens
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    fn now_millis() -> u64 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_new_bucket() {
        let bucket = TokenBucket::new(100, 10.0);
        assert_eq!(bucket.available(), 100);
    }

    #[test]
    fn test_consume_one() {
        let bucket = TokenBucket::new(10, 1.0);
        assert!(bucket.try_consume());
        assert_eq!(bucket.available(), 9);
    }

    #[test]
    fn test_consume_all() {
        let bucket = TokenBucket::new(5, 0.1);

        for _ in 0..5 {
            assert!(bucket.try_consume());
        }

        // Should be rate-limited now
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_for_requests_per_minute() {
        let bucket = TokenBucket::for_requests_per_minute(60);
        // Burst capacity is 60/6 = 10
        // Refill rate is 60/60 = 1/sec

        // Consume burst
        for _ in 0..10 {
            assert!(bucket.try_consume());
        }

        // Should be limited
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_retry_after() {
        let bucket = TokenBucket::new(1, 1.0);

        // Consume the only token
        assert!(bucket.try_consume());

        // Should need to wait
        let retry = bucket.retry_after();
        assert!(retry >= 1);
    }

    #[test]
    fn test_refill() {
        let bucket = TokenBucket::new(10, 100.0); // 100 tokens/sec

        // Consume all tokens
        for _ in 0..10 {
            assert!(bucket.try_consume());
        }

        assert!(!bucket.try_consume());

        // Wait a bit for refill
        thread::sleep(Duration::from_millis(50));

        // Should have refilled some tokens
        let available = bucket.available();
        assert!(available > 0, "expected some tokens, got {}", available);
    }
}
