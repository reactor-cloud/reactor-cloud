//! Domain verification via DNS TXT or HTTP challenge.

use crate::error::SitesError;
use reqwest::Client;

/// Verify a domain via DNS TXT record.
pub async fn verify_dns(host: &str, token: &str) -> Result<bool, SitesError> {
    let record_name = format!("_reactor-verify.{}", host);
    let expected_value = format!("reactor-site-verification={}", token);

    let resolver = trust_dns_resolver::TokioAsyncResolver::tokio_from_system_conf()
        .map_err(|e| SitesError::DomainVerificationFailed(e.to_string()))?;

    match resolver.txt_lookup(&record_name).await {
        Ok(response) => {
            for record in response.iter() {
                let value: String = record.iter().map(|d| String::from_utf8_lossy(d)).collect();
                if value.trim() == expected_value {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Err(e) => {
            tracing::debug!("DNS verification failed for {}: {}", host, e);
            Ok(false)
        }
    }
}

/// Verify a domain via HTTP challenge.
pub async fn verify_http(host: &str, token: &str) -> Result<bool, SitesError> {
    let url = format!("http://{}/.well-known/reactor-verify", host);
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| SitesError::DomainVerificationFailed(e.to_string()))?;

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let body = response.text().await.unwrap_or_default();
                Ok(body.trim() == token)
            } else {
                Ok(false)
            }
        }
        Err(e) => {
            tracing::debug!("HTTP verification failed for {}: {}", host, e);
            Ok(false)
        }
    }
}

/// Verify a domain using the specified method.
pub async fn verify_domain(host: &str, token: &str, method: &str) -> Result<bool, SitesError> {
    match method {
        "dns" => verify_dns(host, token).await,
        "http" => verify_http(host, token).await,
        _ => Err(SitesError::DomainVerificationFailed(format!(
            "unknown verification method: {}",
            method
        ))),
    }
}
