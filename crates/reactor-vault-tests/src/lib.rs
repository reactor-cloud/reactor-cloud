//! Integration test utilities for reactor-vault.
//!
//! This crate provides:
//! - Test container setup for OpenBao/Vault
//! - Property-based testing strategies
//! - Common test fixtures

use reactor_core::ProjectId;
use reactor_vault::{OpenBaoConfig, OpenBaoAuth};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::hashicorp_vault::HashicorpVault;
use url::Url;

/// OpenBao test container configuration.
pub struct OpenBaoTestContainer {
    /// Root token for the test vault
    root_token: String,
    /// Address of the vault
    address: String,
}

impl OpenBaoTestContainer {
    /// Start a new OpenBao/Vault test container.
    ///
    /// Returns the container info. Caller should keep it in scope for the duration of tests.
    pub async fn start() -> (Self, testcontainers::ContainerAsync<HashicorpVault>) {
        // Use the testcontainers-modules vault image (runs in dev mode)
        let vault = HashicorpVault::default();

        let container = vault.start().await.expect("Failed to start Vault container");
        
        let host = container.get_host().await.expect("Failed to get container host");
        let port = container.get_host_port_ipv4(8200).await.expect("Failed to get container port");
        let address = format!("http://{}:{}", host, port);

        // Wait a bit for Vault to be fully ready
        tokio::time::sleep(Duration::from_secs(2)).await;

        // The default dev mode token is "root"
        let info = Self {
            root_token: "root".to_string(),
            address,
        };

        (info, container)
    }

    /// Get the root token for authentication.
    pub fn root_token(&self) -> &str {
        &self.root_token
    }

    /// Get the Vault address.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Build an OpenBaoConfig for this container.
    pub fn config(&self) -> OpenBaoConfig {
        OpenBaoConfig {
            address: Url::parse(&self.address).expect("Invalid address URL"),
            namespace: None,
            kv_mount: "secret".to_string(),
            transit_mount: "transit".to_string(),
            auth: OpenBaoAuth::Token {
                token: self.root_token.clone(),
            },
            ca_cert: None,
        }
    }
}

/// Generate a test project ID.
pub fn test_project_id() -> ProjectId {
    ProjectId::new()
}

/// Generate test data of a specific size.
pub fn test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Property testing strategies for vault operations.
pub mod strategies {
    use proptest::prelude::*;

    /// Strategy for generating secret names.
    pub fn secret_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9-]{0,62}".prop_map(String::from)
    }

    /// Strategy for generating secret values.
    pub fn secret_value() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(any::<u8>(), 1..1024)
    }

    /// Strategy for generating transit key names.
    pub fn transit_key_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9-]{0,30}".prop_map(String::from)
    }

    /// Strategy for generating plaintext data.
    pub fn plaintext() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(any::<u8>(), 1..4096)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_container_starts() {
        let container = OpenBaoTestContainer::start().await;
        assert!(!container.address().is_empty());
        assert_eq!(container.root_token(), "test-root-token");
    }
}
