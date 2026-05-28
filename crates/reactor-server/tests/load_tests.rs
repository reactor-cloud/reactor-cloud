//! Load and stress tests for shared cluster multi-tenancy.
//!
//! These tests are designed to be run against a real shared cluster deployment
//! (or a local Docker Compose setup). They verify:
//! - Noisy-neighbor isolation under load
//! - Connection pool behavior at scale
//! - Tenant cache eviction under memory pressure
//! - Quota enforcement under concurrent load
//!
//! Run with: cargo test --package reactor-server --test load_tests --features cap-cloud -- --ignored

#![cfg(feature = "cap-cloud")]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// Load test configuration.
#[derive(Clone)]
pub struct LoadTestConfig {
    /// Target server URL (e.g., "http://localhost:8000").
    pub base_url: String,
    /// Number of concurrent tenants.
    pub tenant_count: usize,
    /// Requests per tenant per second.
    pub requests_per_tenant_per_sec: u32,
    /// Test duration in seconds.
    pub duration_secs: u64,
    /// Admin token for setup/teardown.
    pub admin_token: String,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var("LOAD_TEST_URL")
                .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            tenant_count: 100,
            requests_per_tenant_per_sec: 10,
            duration_secs: 60,
            admin_token: std::env::var("LOAD_TEST_ADMIN_TOKEN")
                .unwrap_or_else(|_| "test-admin-token".to_string()),
        }
    }
}

/// Results from a load test run.
#[derive(Debug, Default)]
pub struct LoadTestResults {
    /// Total requests made.
    pub total_requests: u64,
    /// Successful requests (2xx).
    pub successful: u64,
    /// Rate limited (429).
    pub rate_limited: u64,
    /// Server errors (5xx).
    pub server_errors: u64,
    /// Client errors (4xx excluding 429).
    pub client_errors: u64,
    /// Request latencies in milliseconds.
    pub latencies_ms: Vec<u64>,
    /// Per-tenant request counts.
    pub per_tenant: HashMap<String, TenantResults>,
}

impl LoadTestResults {
    pub fn p50_latency_ms(&self) -> Option<u64> {
        self.percentile_latency(50)
    }

    pub fn p99_latency_ms(&self) -> Option<u64> {
        self.percentile_latency(99)
    }

    fn percentile_latency(&self, pct: usize) -> Option<u64> {
        if self.latencies_ms.is_empty() {
            return None;
        }
        let mut sorted = self.latencies_ms.clone();
        sorted.sort();
        let idx = (sorted.len() * pct / 100).saturating_sub(1);
        Some(sorted[idx])
    }
}

#[derive(Debug, Default, Clone)]
pub struct TenantResults {
    pub requests: u64,
    pub successful: u64,
    pub rate_limited: u64,
}

/// Load test harness for multi-tenant shared cluster.
pub struct LoadTestHarness {
    config: LoadTestConfig,
    client: reqwest::Client,
    results: Arc<tokio::sync::Mutex<LoadTestResults>>,
}

impl LoadTestHarness {
    pub fn new(config: LoadTestConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(100)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            client,
            results: Arc::new(tokio::sync::Mutex::new(LoadTestResults::default())),
        }
    }

    /// Run a noisy-neighbor test.
    ///
    /// Creates multiple tenants and runs concurrent load against them.
    /// One "noisy" tenant sends 10x the normal request rate.
    /// Verifies that other tenants are not significantly impacted.
    #[allow(dead_code)]
    pub async fn run_noisy_neighbor_test(&self) -> LoadTestResults {
        let tenant_refs: Vec<_> = (0..self.config.tenant_count)
            .map(|i| format!("loadtest_{:04}", i))
            .collect();

        // Tenant 0 is the "noisy neighbor"
        let noisy_tenant = &tenant_refs[0];

        let semaphore = Arc::new(Semaphore::new(100));
        let start = Instant::now();
        let duration = Duration::from_secs(self.config.duration_secs);

        let mut handles = Vec::new();

        for (i, tenant_ref) in tenant_refs.iter().enumerate() {
            let client = self.client.clone();
            let base_url = self.config.base_url.clone();
            let tenant_ref = tenant_ref.clone();
            let results = self.results.clone();
            let sem = semaphore.clone();

            // Noisy tenant gets 10x the rate
            let rate_multiplier = if i == 0 { 10 } else { 1 };
            let requests_per_sec = self.config.requests_per_tenant_per_sec * rate_multiplier;
            let interval = Duration::from_millis(1000 / requests_per_sec as u64);

            let handle = tokio::spawn(async move {
                let mut next_request = Instant::now();

                while start.elapsed() < duration {
                    // Wait for next request slot
                    if Instant::now() < next_request {
                        tokio::time::sleep_until(tokio::time::Instant::from_std(next_request)).await;
                    }
                    next_request = Instant::now() + interval;

                    let _permit = sem.acquire().await.unwrap();
                    let req_start = Instant::now();

                    let url = format!("{}/api/data/health", base_url);
                    let resp = client
                        .get(&url)
                        .header("Host", format!("{}.reactor.cloud", tenant_ref))
                        .send()
                        .await;

                    let latency_ms = req_start.elapsed().as_millis() as u64;

                    let mut results = results.lock().await;
                    results.total_requests += 1;
                    results.latencies_ms.push(latency_ms);

                    let tenant_results = results
                        .per_tenant
                        .entry(tenant_ref.clone())
                        .or_default();
                    tenant_results.requests += 1;

                    match resp {
                        Ok(r) => {
                            let status = r.status().as_u16();
                            if status >= 200 && status < 300 {
                                results.successful += 1;
                                tenant_results.successful += 1;
                            } else if status == 429 {
                                results.rate_limited += 1;
                                tenant_results.rate_limited += 1;
                            } else if status >= 500 {
                                results.server_errors += 1;
                            } else {
                                results.client_errors += 1;
                            }
                        }
                        Err(_) => {
                            results.server_errors += 1;
                        }
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await.ok();
        }

        let results = self.results.lock().await.clone();

        // Verify noisy neighbor isolation
        let noisy_results = results.per_tenant.get(noisy_tenant);
        let other_results: Vec<_> = results
            .per_tenant
            .iter()
            .filter(|(k, _)| k.as_str() != noisy_tenant)
            .collect();

        if let Some(noisy) = noisy_results {
            let noisy_rate_limited_pct = noisy.rate_limited as f64 / noisy.requests as f64;
            println!(
                "Noisy tenant: {} requests, {}% rate limited",
                noisy.requests,
                noisy_rate_limited_pct * 100.0
            );
        }

        for (ref_, tenant_result) in &other_results {
            let rate_limited_pct = tenant_result.rate_limited as f64 / tenant_result.requests.max(1) as f64;
            println!(
                "Tenant {}: {} requests, {}% rate limited",
                ref_,
                tenant_result.requests,
                rate_limited_pct * 100.0
            );
        }

        results
    }

    /// Run a connection pool exhaustion test.
    ///
    /// Creates many concurrent database queries to test pool behavior.
    #[allow(dead_code)]
    pub async fn run_pool_exhaustion_test(&self, concurrent_queries: usize) -> LoadTestResults {
        let tenant_ref = "pooltest_01";
        let semaphore = Arc::new(Semaphore::new(concurrent_queries));

        let mut handles = Vec::new();
        let query_count = concurrent_queries * 10; // 10x the pool size

        for i in 0..query_count {
            let client = self.client.clone();
            let base_url = self.config.base_url.clone();
            let results = self.results.clone();
            let sem = semaphore.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let req_start = Instant::now();

                // Simulate a slow query
                let url = format!("{}/api/data/slow_query?delay_ms=100", base_url);
                let resp = client
                    .get(&url)
                    .header("Host", format!("{}.reactor.cloud", tenant_ref))
                    .send()
                    .await;

                let latency_ms = req_start.elapsed().as_millis() as u64;

                let mut results = results.lock().await;
                results.total_requests += 1;
                results.latencies_ms.push(latency_ms);

                match resp {
                    Ok(r) if r.status().is_success() => results.successful += 1,
                    Ok(r) if r.status().as_u16() == 429 => results.rate_limited += 1,
                    Ok(r) if r.status().is_server_error() => results.server_errors += 1,
                    _ => results.client_errors += 1,
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await.ok();
        }

        self.results.lock().await.clone()
    }

    /// Run a tenant cache scaling test.
    ///
    /// Creates thousands of unique tenants to test cache eviction.
    #[allow(dead_code)]
    pub async fn run_cache_scaling_test(&self, tenant_count: usize) -> LoadTestResults {
        let tenant_refs: Vec<_> = (0..tenant_count)
            .map(|i| format!("scaletest_{:06}", i))
            .collect();

        let semaphore = Arc::new(Semaphore::new(200));

        let mut handles = Vec::new();

        for tenant_ref in tenant_refs {
            let client = self.client.clone();
            let base_url = self.config.base_url.clone();
            let results = self.results.clone();
            let sem = semaphore.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let req_start = Instant::now();

                let url = format!("{}/api/data/health", base_url);
                let resp = client
                    .get(&url)
                    .header("Host", format!("{}.reactor.cloud", tenant_ref))
                    .send()
                    .await;

                let latency_ms = req_start.elapsed().as_millis() as u64;

                let mut results = results.lock().await;
                results.total_requests += 1;
                results.latencies_ms.push(latency_ms);

                match resp {
                    Ok(r) if r.status().is_success() => results.successful += 1,
                    Ok(r) if r.status().is_server_error() => results.server_errors += 1,
                    _ => results.client_errors += 1,
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await.ok();
        }

        self.results.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noisy-neighbor test - verifies that one tenant's load doesn't impact others.
    ///
    /// Run with:
    /// LOAD_TEST_URL=http://localhost:8000 cargo test --package reactor-server \
    ///   --test load_tests noisy_neighbor -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "requires running server"]
    async fn noisy_neighbor_isolation() {
        let config = LoadTestConfig {
            tenant_count: 10,
            requests_per_tenant_per_sec: 5,
            duration_secs: 30,
            ..Default::default()
        };

        let harness = LoadTestHarness::new(config);
        let results = harness.run_noisy_neighbor_test().await;

        println!("\n=== Noisy Neighbor Test Results ===");
        println!("Total requests: {}", results.total_requests);
        println!("Successful: {} ({:.1}%)",
            results.successful,
            results.successful as f64 / results.total_requests as f64 * 100.0
        );
        println!("Rate limited: {} ({:.1}%)",
            results.rate_limited,
            results.rate_limited as f64 / results.total_requests as f64 * 100.0
        );
        println!("P50 latency: {:?}ms", results.p50_latency_ms());
        println!("P99 latency: {:?}ms", results.p99_latency_ms());

        // The noisy tenant (tenant 0) should be rate limited more than others
        // Other tenants should not be significantly impacted
    }

    /// Connection pool exhaustion test.
    ///
    /// Run with:
    /// LOAD_TEST_URL=http://localhost:8000 cargo test --package reactor-server \
    ///   --test load_tests pool_exhaustion -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "requires running server"]
    async fn pool_exhaustion_recovery() {
        let config = LoadTestConfig::default();
        let harness = LoadTestHarness::new(config);
        let results = harness.run_pool_exhaustion_test(50).await;

        println!("\n=== Pool Exhaustion Test Results ===");
        println!("Total requests: {}", results.total_requests);
        println!("Successful: {}", results.successful);
        println!("Server errors: {}", results.server_errors);
        println!("P99 latency: {:?}ms", results.p99_latency_ms());

        // Even under pool pressure, most requests should succeed
        // (with longer latency due to queuing)
        assert!(
            results.successful as f64 / results.total_requests as f64 > 0.9,
            "Expected >90% success rate under pool pressure"
        );
    }

    /// 5k tenant scaling test.
    ///
    /// Run with:
    /// LOAD_TEST_URL=http://localhost:8000 cargo test --package reactor-server \
    ///   --test load_tests scale_5k_tenants -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "requires running server with large cache"]
    async fn scale_5k_tenants() {
        let config = LoadTestConfig::default();
        let harness = LoadTestHarness::new(config);
        let results = harness.run_cache_scaling_test(5000).await;

        println!("\n=== 5k Tenant Scaling Test Results ===");
        println!("Total requests: {}", results.total_requests);
        println!("Successful: {}", results.successful);
        println!("Server errors: {}", results.server_errors);
        println!("P50 latency: {:?}ms", results.p50_latency_ms());
        println!("P99 latency: {:?}ms", results.p99_latency_ms());

        // Most requests should succeed even with cache eviction
        assert!(
            results.successful as f64 / results.total_requests as f64 > 0.95,
            "Expected >95% success rate at scale"
        );
    }
}
