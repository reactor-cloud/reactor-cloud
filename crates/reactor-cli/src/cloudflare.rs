//! Cloudflare DNS client for subdomain provisioning.
//!
//! Provides a thin wrapper around the Cloudflare DNS API for creating,
//! listing, and deleting DNS records on the `reactor.cloud` zone.

use crate::error::{CliError, CliResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default zone for reactor.cloud subdomains.
pub const DEFAULT_ZONE: &str = "reactor.cloud";

/// Default edge target (Fly.io edge gateway).
/// TODO: Move to context config when we go multi-region (M6).
pub const DEFAULT_EDGE_TARGET: &str = "rc-shared-1-edge.fly.dev";

/// Cloudflare API base URL.
const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare API response wrapper.
#[derive(Debug, Deserialize)]
struct CfResponse<T> {
    success: bool,
    errors: Vec<CfError>,
    result: Option<T>,
}

/// Cloudflare API error.
#[derive(Debug, Deserialize)]
struct CfError {
    #[allow(dead_code)]
    code: i32,
    message: String,
}

/// Cloudflare zone info.
#[derive(Debug, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
}

/// Cloudflare DNS record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub content: String,
    pub proxied: bool,
    pub ttl: u32,
}

/// Input for creating a DNS record.
#[derive(Debug, Serialize)]
struct CreateRecordInput {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    proxied: bool,
    ttl: u32,
}

/// Resolve the Cloudflare API token.
///
/// Precedence:
/// 1. `cli_token` argument (from `--cf-token` flag)
/// 2. `CF_API_TOKEN` env var
/// 3. Load `.env` from CWD (walking up to git root), then re-check `CF_API_TOKEN`
pub fn resolve_token(cli_token: Option<&str>) -> CliResult<String> {
    // 1. CLI flag
    if let Some(token) = cli_token {
        if !token.is_empty() {
            return Ok(token.to_string());
        }
    }

    // 2. Env var (before .env load)
    if let Ok(token) = std::env::var("CF_API_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // 3. Load .env walking up from CWD
    if let Some(env_path) = find_dotenv() {
        let _ = dotenvy::from_path(&env_path);

        // Re-check env var after loading
        if let Ok(token) = std::env::var("CF_API_TOKEN") {
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }

    Err(CliError::Config(
        "CF_API_TOKEN not set; export it or add to .env".into(),
    ))
}

/// Resolve the zone name.
///
/// Returns `CF_ZONE` env var or `DEFAULT_ZONE`.
pub fn resolve_zone() -> String {
    std::env::var("CF_ZONE").unwrap_or_else(|_| DEFAULT_ZONE.to_string())
}

/// Resolve the edge target.
///
/// Returns `REACTOR_EDGE_TARGET` env var or `DEFAULT_EDGE_TARGET`.
pub fn resolve_edge_target() -> String {
    std::env::var("REACTOR_EDGE_TARGET").unwrap_or_else(|_| DEFAULT_EDGE_TARGET.to_string())
}

/// Find a `.env` file walking up from CWD to the repo root.
fn find_dotenv() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut dir = cwd.as_path();

    loop {
        let env_path = dir.join(".env");
        if env_path.exists() {
            return Some(env_path);
        }

        // Stop at git root
        if dir.join(".git").exists() {
            return None;
        }

        dir = dir.parent()?;
    }
}

/// Normalize a subdomain input to a bare label.
///
/// Examples:
/// - `antennanew` -> `antennanew`
/// - `antennanew.reactor.cloud` -> `antennanew`
/// - `antennanew.reactor.cloud.` -> `antennanew`
pub fn normalize_subdomain(input: &str, zone: &str) -> String {
    let input = input.trim_end_matches('.');
    let zone_suffix = format!(".{}", zone);
    if input.ends_with(&zone_suffix) {
        input[..input.len() - zone_suffix.len()].to_string()
    } else {
        input.to_string()
    }
}

/// Build the FQDN from a subdomain label and zone.
pub fn fqdn(label: &str, zone: &str) -> String {
    format!("{}.{}", label, zone)
}

/// Cloudflare DNS client.
pub struct CloudflareClient {
    client: reqwest::Client,
    token: String,
}

impl CloudflareClient {
    /// Create a new client with the given API token.
    pub fn new(token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
        }
    }

    /// Get the zone ID for a zone name.
    pub async fn get_zone_id(&self, zone_name: &str) -> CliResult<String> {
        let url = format!("{}/zones?name={}", CF_API_BASE, zone_name);

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| CliError::Network(format!("Cloudflare API request failed: {}", e)))?;

        let status = resp.status();
        let body: CfResponse<Vec<Zone>> = resp
            .json()
            .await
            .map_err(|e| CliError::Network(format!("Failed to parse Cloudflare response: {}", e)))?;

        if !body.success {
            let msg = body
                .errors
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| format!("HTTP {}", status));
            return Err(CliError::Server(format!("Cloudflare API error: {}", msg)));
        }

        body.result
            .and_then(|zones| zones.into_iter().next())
            .map(|z| z.id)
            .ok_or_else(|| {
                CliError::Config(format!(
                    "Zone '{}' not found or not accessible with this token",
                    zone_name
                ))
            })
    }

    /// Create a CNAME DNS record.
    pub async fn create_record(
        &self,
        zone_id: &str,
        name: &str,
        target: &str,
        proxied: bool,
    ) -> CliResult<DnsRecord> {
        let url = format!("{}/zones/{}/dns_records", CF_API_BASE, zone_id);

        let input = CreateRecordInput {
            record_type: "CNAME".to_string(),
            name: name.to_string(),
            content: target.to_string(),
            proxied,
            ttl: 1, // Auto TTL
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&input)
            .send()
            .await
            .map_err(|e| CliError::Network(format!("Cloudflare API request failed: {}", e)))?;

        let status = resp.status();
        let body: CfResponse<DnsRecord> = resp
            .json()
            .await
            .map_err(|e| CliError::Network(format!("Failed to parse Cloudflare response: {}", e)))?;

        if !body.success {
            let msg = body
                .errors
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| format!("HTTP {}", status));

            // Check for duplicate record error
            if msg.contains("already exists") {
                return Err(CliError::Validation(format!(
                    "DNS record '{}' already exists",
                    name
                )));
            }

            return Err(CliError::Server(format!("Cloudflare API error: {}", msg)));
        }

        body.result.ok_or_else(|| {
            CliError::Server("Cloudflare API returned success but no record".into())
        })
    }

    /// List DNS records, optionally filtered by name.
    pub async fn list_records(
        &self,
        zone_id: &str,
        name_filter: Option<&str>,
    ) -> CliResult<Vec<DnsRecord>> {
        let mut url = format!(
            "{}/zones/{}/dns_records?type=CNAME&per_page=100",
            CF_API_BASE, zone_id
        );

        if let Some(name) = name_filter {
            url.push_str(&format!("&name={}", name));
        }

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| CliError::Network(format!("Cloudflare API request failed: {}", e)))?;

        let status = resp.status();
        let body: CfResponse<Vec<DnsRecord>> = resp
            .json()
            .await
            .map_err(|e| CliError::Network(format!("Failed to parse Cloudflare response: {}", e)))?;

        if !body.success {
            let msg = body
                .errors
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| format!("HTTP {}", status));
            return Err(CliError::Server(format!("Cloudflare API error: {}", msg)));
        }

        Ok(body.result.unwrap_or_default())
    }

    /// Delete a DNS record by ID.
    pub async fn delete_record(&self, zone_id: &str, record_id: &str) -> CliResult<()> {
        let url = format!(
            "{}/zones/{}/dns_records/{}",
            CF_API_BASE, zone_id, record_id
        );

        let resp = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| CliError::Network(format!("Cloudflare API request failed: {}", e)))?;

        let status = resp.status();
        let body: CfResponse<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| CliError::Network(format!("Failed to parse Cloudflare response: {}", e)))?;

        if !body.success {
            let msg = body
                .errors
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| format!("HTTP {}", status));
            return Err(CliError::Server(format!("Cloudflare API error: {}", msg)));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_subdomain() {
        assert_eq!(normalize_subdomain("antennanew", "reactor.cloud"), "antennanew");
        assert_eq!(
            normalize_subdomain("antennanew.reactor.cloud", "reactor.cloud"),
            "antennanew"
        );
        assert_eq!(
            normalize_subdomain("antennanew.reactor.cloud.", "reactor.cloud"),
            "antennanew"
        );
        assert_eq!(
            normalize_subdomain("sub.antennanew.reactor.cloud", "reactor.cloud"),
            "sub.antennanew"
        );
    }

    #[test]
    fn test_fqdn() {
        assert_eq!(fqdn("antennanew", "reactor.cloud"), "antennanew.reactor.cloud");
    }
}
