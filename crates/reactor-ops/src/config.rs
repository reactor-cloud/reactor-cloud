//! Configuration for the ops control surface.

use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};

/// Configuration for the ops control surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsConfig {
    /// Whether the ops surface is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Trusted networks that can access the ops surface.
    /// Requests from other networks will be rejected.
    /// Default: loopback (127.0.0.0/8, ::1/128) and Fly.io 6PN (fdaa::/16)
    #[serde(default = "default_trusted_networks")]
    pub trusted_networks: Vec<String>,

    /// Session TTL in seconds for ops sessions.
    #[serde(default = "default_session_ttl_secs")]
    pub session_ttl_secs: u64,

    /// Step-up authentication window in seconds.
    /// If mfa_at is older than this, step-up is required for flagged operations.
    #[serde(default = "default_step_up_window_secs")]
    pub step_up_window_secs: u64,

    /// Scopes that require step-up authentication.
    #[serde(default = "default_require_step_up_for")]
    pub require_step_up_for: Vec<String>,

    /// Audit log retention in days.
    #[serde(default = "default_audit_retention_days")]
    pub audit_retention_days: u32,
}

fn default_enabled() -> bool {
    true
}

fn default_trusted_networks() -> Vec<String> {
    vec![
        "127.0.0.0/8".to_string(),    // IPv4 loopback
        "::1/128".to_string(),        // IPv6 loopback
        "fdaa::/16".to_string(),      // Fly.io 6PN
    ]
}

fn default_session_ttl_secs() -> u64 {
    1800 // 30 minutes
}

fn default_step_up_window_secs() -> u64 {
    300 // 5 minutes
}

fn default_require_step_up_for() -> Vec<String> {
    vec![
        "ops:cluster_admin".to_string(),
        "vault:write".to_string(),
        "cloud:projects:delete".to_string(),
    ]
}

fn default_audit_retention_days() -> u32 {
    365
}

impl Default for OpsConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            trusted_networks: default_trusted_networks(),
            session_ttl_secs: default_session_ttl_secs(),
            step_up_window_secs: default_step_up_window_secs(),
            require_step_up_for: default_require_step_up_for(),
            audit_retention_days: default_audit_retention_days(),
        }
    }
}

impl OpsConfig {
    /// Parse trusted networks into `IpNetwork` instances.
    pub fn parsed_trusted_networks(&self) -> Vec<IpNetwork> {
        self.trusted_networks
            .iter()
            .filter_map(|s| s.parse::<IpNetwork>().ok())
            .collect()
    }

    /// Check if a given IP address is in the trusted networks.
    pub fn is_trusted_ip(&self, ip: &std::net::IpAddr) -> bool {
        let networks = self.parsed_trusted_networks();
        networks.iter().any(|net| net.contains(*ip))
    }

    /// Check if a scope requires step-up authentication.
    pub fn requires_step_up(&self, scope: &str) -> bool {
        self.require_step_up_for.iter().any(|s| s == scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OpsConfig::default();
        assert!(config.enabled);
        assert!(!config.trusted_networks.is_empty());
    }

    #[test]
    fn test_is_trusted_ip() {
        let config = OpsConfig::default();
        
        // Loopback should be trusted
        assert!(config.is_trusted_ip(&"127.0.0.1".parse().unwrap()));
        assert!(config.is_trusted_ip(&"::1".parse().unwrap()));
        
        // Fly 6PN should be trusted
        assert!(config.is_trusted_ip(&"fdaa::1".parse().unwrap()));
        
        // Public IP should not be trusted
        assert!(!config.is_trusted_ip(&"8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn test_requires_step_up() {
        let config = OpsConfig::default();
        
        assert!(config.requires_step_up("ops:cluster_admin"));
        assert!(config.requires_step_up("vault:write"));
        assert!(!config.requires_step_up("ops:deploy"));
    }
}
