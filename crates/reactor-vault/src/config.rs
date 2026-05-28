//! Vault configuration types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

/// Unified vault configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "backend")]
pub enum VaultConfig {
    /// Embedded file-based vault (for development/single-node).
    #[serde(rename = "embedded")]
    Embedded(EmbeddedConfig),

    /// OpenBao/Vault backend (for production).
    #[serde(rename = "openbao")]
    OpenBao(OpenBaoConfig),
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self::Embedded(EmbeddedConfig::default())
    }
}

/// Configuration for the embedded (file-based) vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedConfig {
    /// Directory to store encrypted secrets.
    pub path: PathBuf,

    /// Master key for encryption (should come from secure source).
    ///
    /// This can be:
    /// - A raw key (32 bytes hex-encoded)
    /// - An environment variable reference: `env:VAULT_MASTER_KEY`
    #[serde(default)]
    pub master_key: String,
}

impl Default for EmbeddedConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("/data/vault"),
            master_key: String::new(),
        }
    }
}

/// Configuration for OpenBao/Vault backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenBaoConfig {
    /// OpenBao server address.
    pub address: Url,

    /// Namespace for multi-tenant isolation.
    #[serde(default)]
    pub namespace: Option<String>,

    /// Mount path for the KV v2 secrets engine.
    #[serde(default = "default_kv_mount")]
    pub kv_mount: String,

    /// Mount path for the transit secrets engine.
    #[serde(default = "default_transit_mount")]
    pub transit_mount: String,

    /// Authentication method.
    pub auth: OpenBaoAuth,

    /// TLS certificate for verification (optional, for self-signed certs).
    #[serde(default)]
    pub ca_cert: Option<PathBuf>,
}

fn default_kv_mount() -> String {
    "secret".to_string()
}

fn default_transit_mount() -> String {
    "transit".to_string()
}

/// OpenBao authentication methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum OpenBaoAuth {
    /// Token-based authentication (for development).
    #[serde(rename = "token")]
    Token {
        /// The Vault token.
        token: String,
    },

    /// AppRole authentication (for production).
    #[serde(rename = "approle")]
    AppRole {
        /// AppRole role ID.
        role_id: String,

        /// Path to file containing the secret ID.
        /// Using a file allows for secret rotation without config changes.
        secret_id_file: PathBuf,

        /// AppRole mount path (default: "approle").
        #[serde(default = "default_approle_mount")]
        mount_path: String,
    },
}

fn default_approle_mount() -> String {
    "approle".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_config_deserialize_embedded() {
        let toml = r#"
            backend = "embedded"
            path = "/data/vault"
            master_key = "env:VAULT_MASTER_KEY"
        "#;

        let config: VaultConfig = toml::from_str(toml).unwrap();
        match config {
            VaultConfig::Embedded(cfg) => {
                assert_eq!(cfg.path, PathBuf::from("/data/vault"));
                assert_eq!(cfg.master_key, "env:VAULT_MASTER_KEY");
            }
            _ => panic!("expected embedded config"),
        }
    }

    #[test]
    fn test_vault_config_deserialize_openbao() {
        let toml = r#"
            backend = "openbao"
            address = "http://localhost:8200"
            namespace = "reactor"
            
            [auth]
            method = "token"
            token = "hvs.xxxxx"
        "#;

        let config: VaultConfig = toml::from_str(toml).unwrap();
        match config {
            VaultConfig::OpenBao(cfg) => {
                assert_eq!(cfg.address.as_str(), "http://localhost:8200/");
                assert_eq!(cfg.namespace, Some("reactor".to_string()));
                match cfg.auth {
                    OpenBaoAuth::Token { token } => {
                        assert_eq!(token, "hvs.xxxxx");
                    }
                    _ => panic!("expected token auth"),
                }
            }
            _ => panic!("expected openbao config"),
        }
    }
}
