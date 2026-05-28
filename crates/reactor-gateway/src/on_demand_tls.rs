//! On-demand TLS endpoint for Caddy.
//!
//! This module provides the `/ask` endpoint that Caddy calls when deciding
//! whether to provision a certificate for a domain on-demand.

use crate::error::GatewayResult;
use async_trait::async_trait;
use tracing::{debug, warn};

/// Trait for checking if a domain is allowed for on-demand TLS.
#[async_trait]
pub trait DomainVerifier: Send + Sync {
    /// Check if the domain is allowed for on-demand TLS.
    ///
    /// Returns `Ok(true)` if the domain has been verified and can receive a certificate.
    /// Returns `Ok(false)` if the domain is not allowed.
    async fn is_domain_allowed(&self, domain: &str) -> GatewayResult<bool>;
}

/// On-demand TLS handler.
pub struct OnDemandTlsHandler<V: DomainVerifier> {
    verifier: V,
}

impl<V: DomainVerifier> OnDemandTlsHandler<V> {
    /// Create a new handler with the given domain verifier.
    pub fn new(verifier: V) -> Self {
        Self { verifier }
    }

    /// Handle the /ask endpoint.
    ///
    /// Returns `true` if Caddy should provision a certificate for this domain.
    pub async fn handle_ask(&self, domain: &str) -> GatewayResult<bool> {
        debug!("On-demand TLS check for domain: {}", domain);

        let allowed = self.verifier.is_domain_allowed(domain).await?;

        if allowed {
            debug!("Domain {} is allowed for on-demand TLS", domain);
        } else {
            warn!("Domain {} is NOT allowed for on-demand TLS", domain);
        }

        Ok(allowed)
    }
}

/// Response for the /ask endpoint.
#[derive(Debug)]
pub enum AskResponse {
    /// Allow certificate provisioning (200 OK).
    Allow,
    /// Deny certificate provisioning (404 Not Found or 403 Forbidden).
    Deny,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockVerifier {
        allowed: Vec<String>,
    }

    #[async_trait]
    impl DomainVerifier for MockVerifier {
        async fn is_domain_allowed(&self, domain: &str) -> GatewayResult<bool> {
            Ok(self.allowed.contains(&domain.to_string()))
        }
    }

    #[tokio::test]
    async fn test_allowed_domain() {
        let verifier = MockVerifier {
            allowed: vec!["allowed.example.com".to_string()],
        };
        let handler = OnDemandTlsHandler::new(verifier);

        assert!(handler.handle_ask("allowed.example.com").await.unwrap());
        assert!(!handler.handle_ask("denied.example.com").await.unwrap());
    }
}
