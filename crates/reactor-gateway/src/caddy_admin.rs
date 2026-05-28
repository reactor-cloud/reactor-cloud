//! Caddy admin API client and JSON config builder.
//!
//! This module provides:
//! - A client for Caddy's admin API (typically on localhost:2019)
//! - JSON config builder for generating Caddy configurations
//! - Atomic config updates

use crate::error::{GatewayError, GatewayResult};
use crate::routing::{BackendKind, Route, RoutingTable};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};
use url::Url;

/// Configuration for the Caddy admin client.
#[derive(Debug, Clone)]
pub struct CaddyAdminConfig {
    /// Caddy admin API address.
    pub admin_address: Url,
    /// Cloudflare API token for DNS-01 challenges.
    pub cloudflare_token: Option<String>,
    /// Default backend for unmatched requests.
    pub default_backend: Option<String>,
    /// Wildcard domain (e.g., "*.reactor.cloud").
    pub wildcard_domain: String,
    /// ACME email for Let's Encrypt.
    pub acme_email: String,
    /// Use Let's Encrypt staging for testing.
    pub acme_staging: bool,
    /// Rate limiting configuration for defense-in-depth.
    pub rate_limit: Option<RateLimitConfig>,
}

/// Rate limiting configuration for Caddy (defense-in-depth).
///
/// This provides edge-level rate limiting as a fallback to application-level
/// quotas. It protects against:
/// - DDoS attacks before they reach the application
/// - Quota bypass attempts
/// - Noisy neighbors in shared clusters
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per second per host (0 = disabled).
    pub requests_per_second: u32,
    /// Burst size (allows short spikes above rate).
    pub burst: u32,
    /// Enable rate limiting for shared backend routes only.
    /// Dedicated routes get unlimited edge rate limiting.
    pub shared_only: bool,
}

impl CaddyAdminConfig {
    /// Create a new config with defaults.
    pub fn new(admin_address: Url) -> Self {
        Self {
            admin_address,
            cloudflare_token: None,
            default_backend: None,
            wildcard_domain: "*.reactor.cloud".to_string(),
            acme_email: "admin@reactor.cloud".to_string(),
            acme_staging: false,
            rate_limit: None,
        }
    }

    /// Set the Cloudflare API token.
    pub fn with_cloudflare_token(mut self, token: impl Into<String>) -> Self {
        self.cloudflare_token = Some(token.into());
        self
    }

    /// Set the default backend.
    pub fn with_default_backend(mut self, backend: impl Into<String>) -> Self {
        self.default_backend = Some(backend.into());
        self
    }

    /// Set the wildcard domain.
    pub fn with_wildcard_domain(mut self, domain: impl Into<String>) -> Self {
        self.wildcard_domain = domain.into();
        self
    }

    /// Set the ACME email.
    pub fn with_acme_email(mut self, email: impl Into<String>) -> Self {
        self.acme_email = email.into();
        self
    }

    /// Enable ACME staging.
    pub fn with_acme_staging(mut self) -> Self {
        self.acme_staging = true;
        self
    }

    /// Configure rate limiting for defense-in-depth.
    pub fn with_rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit = Some(config);
        self
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
            burst: 200,
            shared_only: true,
        }
    }
}

/// Caddy admin API client.
pub struct CaddyAdminClient {
    client: Client,
    config: CaddyAdminConfig,
}

impl CaddyAdminClient {
    /// Create a new Caddy admin client.
    pub fn new(config: CaddyAdminConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Get the current Caddy configuration.
    pub async fn get_config(&self) -> GatewayResult<Value> {
        let url = format!("{}/config/", self.config.admin_address);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GatewayError::caddy_admin(format!(
                "Failed to get config: {} - {}",
                status, body
            )));
        }

        let config = response.json().await?;
        Ok(config)
    }

    /// Load a new Caddy configuration atomically.
    pub async fn load_config(&self, config: &Value) -> GatewayResult<()> {
        let url = format!("{}/load", self.config.admin_address);
        debug!("Loading Caddy config via {}", url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(config)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GatewayError::caddy_admin(format!(
                "Failed to load config: {} - {}",
                status, body
            )));
        }

        info!("Successfully loaded Caddy config");
        Ok(())
    }

    /// Build a complete Caddy JSON configuration from the routing table.
    pub fn build_config(&self, table: &RoutingTable) -> GatewayResult<Value> {
        let builder = CaddyConfigBuilder::new(&self.config);
        builder.build(table)
    }

    /// Apply the routing table to Caddy.
    pub async fn apply(&self, table: &RoutingTable) -> GatewayResult<()> {
        let config = self.build_config(table)?;
        self.load_config(&config).await
    }

    /// Check if Caddy is healthy.
    pub async fn health_check(&self) -> GatewayResult<bool> {
        let url = format!("{}/config/", self.config.admin_address);
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => {
                warn!("Caddy health check failed: {}", e);
                Ok(false)
            }
        }
    }
}

/// Builder for Caddy JSON configurations.
pub struct CaddyConfigBuilder<'a> {
    config: &'a CaddyAdminConfig,
}

impl<'a> CaddyConfigBuilder<'a> {
    /// Create a new config builder.
    pub fn new(config: &'a CaddyAdminConfig) -> Self {
        Self { config }
    }

    /// Build the complete Caddy JSON configuration.
    pub fn build(&self, table: &RoutingTable) -> GatewayResult<Value> {
        let mut config = self.build_base_config();

        // Build apps section
        let apps = self.build_apps(table)?;
        config["apps"] = apps;

        Ok(config)
    }

    /// Build the base Caddy configuration.
    fn build_base_config(&self) -> Value {
        json!({
            "admin": {
                "listen": "localhost:2019",
                "enforce_origin": false
            },
            "logging": {
                "logs": {
                    "default": {
                        "level": "INFO",
                        "encoder": {
                            "format": "json"
                        }
                    }
                }
            }
        })
    }

    /// Build the apps section.
    fn build_apps(&self, table: &RoutingTable) -> GatewayResult<Value> {
        let http = self.build_http_app(table)?;
        let tls = self.build_tls_app()?;

        Ok(json!({
            "http": http,
            "tls": tls
        }))
    }

    /// Build the HTTP app configuration.
    fn build_http_app(&self, table: &RoutingTable) -> GatewayResult<Value> {
        let servers = self.build_servers(table)?;

        Ok(json!({
            "servers": servers
        }))
    }

    /// Build the servers configuration.
    fn build_servers(&self, table: &RoutingTable) -> GatewayResult<Value> {
        let routes = self.build_routes(table)?;

        // Main HTTPS server
        let https_server = json!({
            "listen": [":443"],
            "routes": routes,
            "automatic_https": {
                "disable": false
            }
        });

        // HTTP redirect server
        let http_server = json!({
            "listen": [":80"],
            "routes": [{
                "match": [{
                    "host": ["*"]
                }],
                "handle": [{
                    "handler": "static_response",
                    "status_code": "301",
                    "headers": {
                        "Location": ["https://{http.request.host}{http.request.uri}"]
                    }
                }]
            }]
        });

        Ok(json!({
            "https": https_server,
            "http": http_server
        }))
    }

    /// Build route configurations.
    fn build_routes(&self, table: &RoutingTable) -> GatewayResult<Vec<Value>> {
        let mut routes = Vec::new();

        // Group routes by backend target for efficiency
        let mut target_routes: HashMap<String, Vec<&Route>> = HashMap::new();
        for route in table.enabled() {
            target_routes
                .entry(route.backend_target.address.clone())
                .or_default()
                .push(route);
        }

        // Build individual routes
        for route in table.enabled() {
            routes.push(self.build_route(route)?);
        }

        // Add default backend route if configured
        if let Some(ref default_backend) = self.config.default_backend {
            routes.push(json!({
                "match": [{
                    "host": ["*"]
                }],
                "handle": [{
                    "handler": "reverse_proxy",
                    "upstreams": [{
                        "dial": default_backend
                    }]
                }],
                "terminal": true
            }));
        }

        Ok(routes)
    }

    /// Build a single route configuration.
    fn build_route(&self, route: &Route) -> GatewayResult<Value> {
        let mut handlers = Vec::new();

        // Add rate limiting for shared backends (defense-in-depth)
        if let Some(ref rate_limit) = self.config.rate_limit {
            let should_rate_limit = if rate_limit.shared_only {
                route.backend_kind == BackendKind::Shared
            } else {
                true
            };

            if should_rate_limit && rate_limit.requests_per_second > 0 {
                handlers.push(self.build_rate_limiter(rate_limit, &route.host)?);
            }
        }

        // Add reverse proxy handler with X-Reactor-Project header
        handlers.push(self.build_reverse_proxy(
            &route.backend_target.address,
            &route.project_ref,
        )?);

        Ok(json!({
            "match": [{
                "host": [&route.host]
            }],
            "handle": handlers,
            "terminal": true
        }))
    }

    /// Build a rate limiter handler.
    ///
    /// Uses Caddy's rate_limit handler to enforce per-host request limits.
    /// This is defense-in-depth - application quotas are the primary limit.
    fn build_rate_limiter(&self, config: &RateLimitConfig, host: &str) -> GatewayResult<Value> {
        Ok(json!({
            "handler": "rate_limit",
            "rate_limits": {
                "static": {
                    "match": [{
                        "methods": ["GET", "POST", "PUT", "PATCH", "DELETE"]
                    }],
                    "key": host,
                    "window": "1s",
                    "max_events": config.requests_per_second
                }
            },
            "distributed": {
                "write_interval": "1s"
            },
            "jitter": 0.2
        }))
    }

    /// Build a reverse proxy handler.
    fn build_reverse_proxy(
        &self,
        backend: &str,
        project_ref: &reactor_core::ProjectRef,
    ) -> GatewayResult<Value> {
        Ok(json!({
            "handler": "reverse_proxy",
            "upstreams": [{
                "dial": backend
            }],
            "headers": {
                "request": {
                    "set": {
                        "X-Forwarded-Proto": ["https"],
                        "X-Real-IP": ["{http.request.remote.host}"],
                        "X-Reactor-Project": [project_ref.as_str()]
                    }
                }
            },
            "transport": {
                "protocol": "http",
                "tls": {}
            }
        }))
    }

    /// Build the TLS app configuration.
    fn build_tls_app(&self) -> GatewayResult<Value> {
        let mut automation = json!({
            "on_demand": {
                "ask": "http://localhost:9000/ask",
                "interval": "2m",
                "burst": 5
            }
        });

        // Add ACME configuration
        let mut policies = vec![];

        // Wildcard policy using DNS-01 challenge
        if let Some(ref cf_token) = self.config.cloudflare_token {
            policies.push(json!({
                "subjects": [&self.config.wildcard_domain, self.config.wildcard_domain.trim_start_matches("*.")],
                "issuers": [{
                    "module": "acme",
                    "email": &self.config.acme_email,
                    "challenges": {
                        "dns": {
                            "provider": {
                                "name": "cloudflare",
                                "api_token": cf_token
                            }
                        }
                    }
                }]
            }));
        }

        // On-demand policy for custom domains (HTTP-01 challenge)
        policies.push(json!({
            "on_demand": true,
            "issuers": [{
                "module": "acme",
                "email": &self.config.acme_email
            }]
        }));

        if self.config.acme_staging {
            for policy in &mut policies {
                if let Some(issuers) = policy.get_mut("issuers") {
                    if let Some(arr) = issuers.as_array_mut() {
                        for issuer in arr {
                            issuer["ca"] = json!("https://acme-staging-v02.api.letsencrypt.org/directory");
                        }
                    }
                }
            }
        }

        automation["policies"] = json!(policies);

        Ok(json!({
            "automation": automation
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::{BackendTarget, Route};
    use reactor_core::{ProjectId, ProjectRef};

    fn test_config() -> CaddyAdminConfig {
        CaddyAdminConfig::new(Url::parse("http://localhost:2019").unwrap())
            .with_cloudflare_token("test-token")
            .with_default_backend("reactor-cloud.internal:8000")
    }

    #[test]
    fn test_build_empty_config() {
        let config = test_config();
        let builder = CaddyConfigBuilder::new(&config);
        let table = RoutingTable::new();

        let result = builder.build(&table).unwrap();
        assert!(result["apps"]["http"]["servers"].is_object());
        assert!(result["apps"]["tls"]["automation"].is_object());
    }

    #[test]
    fn test_build_with_routes() {
        let config = test_config();
        let builder = CaddyConfigBuilder::new(&config);

        let project_id = ProjectId::new();
        let project_ref = ProjectRef::from(&project_id);
        let target = BackendTarget::new("backend:8000");
        let route = Route::new("test.reactor.cloud", project_id, project_ref, target);

        let mut table = RoutingTable::new();
        table.upsert(route);

        let result = builder.build(&table).unwrap();
        let routes = result["apps"]["http"]["servers"]["https"]["routes"]
            .as_array()
            .unwrap();

        // Should have at least the route we added + default backend
        assert!(!routes.is_empty());
    }

    #[test]
    fn test_on_demand_tls_config() {
        let config = test_config();
        let builder = CaddyConfigBuilder::new(&config);

        let tls = builder.build_tls_app().unwrap();
        let on_demand = &tls["automation"]["on_demand"];

        assert_eq!(on_demand["ask"], "http://localhost:9000/ask");
    }
}
