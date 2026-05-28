//! Admin endpoints (`/_admin/*`).

use crate::error::ClientResult;
use crate::http::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Server version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub reactor_server: String,
    pub capabilities: HashMap<String, String>,
}

/// Doctor check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorResult {
    pub capabilities: HashMap<String, CapabilityHealth>,
}

/// Health status for a single capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityHealth {
    pub status: String,
    #[serde(default)]
    pub details: HashMap<String, serde_json::Value>,
}

/// Migration result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateResult {
    pub applied: Vec<String>,
    pub skipped: Vec<String>,
}

/// Deploy result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResult {
    pub deploy_id: String,
    pub status: DeployStatus,
    pub phases: Vec<DeployPhase>,
}

/// Deploy status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeployStatus {
    Ok,
    Partial,
    Failed,
}

/// A single phase of a deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPhase {
    pub capability: String,
    pub status: String,
    #[serde(default)]
    pub details: HashMap<String, serde_json::Value>,
}

impl Client {
    /// Get server version information.
    pub async fn version(&self) -> ClientResult<VersionInfo> {
        self.get("/_admin/version").await
    }

    /// Run doctor checks.
    pub async fn doctor(&self) -> ClientResult<DoctorResult> {
        self.get("/_admin/doctor").await
    }

    /// Run migrations.
    pub async fn migrate(&self, dry_run: bool) -> ClientResult<MigrateResult> {
        let path = if dry_run {
            "/_admin/migrate?dry_run=true"
        } else {
            "/_admin/migrate"
        };
        self.post(path, &()).await
    }

    /// Deploy a bundle.
    pub async fn deploy(&self, bundle: Vec<u8>) -> ClientResult<DeployResult> {
        use reqwest::multipart::{Form, Part};

        let part = Part::bytes(bundle)
            .file_name("deploy.tar.zst")
            .mime_str("application/zstd")?;
        let form = Form::new().part("bundle", part);

        self.post_multipart("/_ops/v1/deployments", form).await
    }

    /// Request graceful shutdown.
    pub async fn shutdown(&self) -> ClientResult<()> {
        self.post::<serde_json::Value, _>("/_admin/shutdown", &())
            .await?;
        Ok(())
    }

    /// Get health status.
    pub async fn health(&self) -> ClientResult<serde_json::Value> {
        self.get("/health").await
    }

    /// Request graceful shutdown (alias for shutdown).
    pub async fn admin_shutdown(&self) -> ClientResult<()> {
        self.shutdown().await
    }

    /// Get server logs.
    pub async fn admin_logs(
        &self,
        since: Option<&str>,
        limit: Option<u32>,
    ) -> ClientResult<Vec<LogEntry>> {
        let mut path = "/_admin/logs".to_string();
        let mut params = vec![];
        if let Some(s) = since {
            params.push(format!("since={}", s));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if !params.is_empty() {
            path.push('?');
            path.push_str(&params.join("&"));
        }
        self.get(&path).await
    }
}

/// Log entry from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

// =============================================================================
// Vault types
// =============================================================================

/// Secret info for list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSecretInfo {
    pub name: String,
    pub version: u64,
}

/// List secrets response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultListResponse {
    pub secrets: Vec<VaultSecretInfo>,
}

/// Get secret response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultGetResponse {
    pub key: String,
    pub value: String,
    pub is_base64: bool,
    pub version: u64,
}

/// Set secret request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSetRequest {
    pub value: String,
    #[serde(default)]
    pub is_base64: bool,
}

/// Rotate key response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultRotateResponse {
    pub status: String,
    pub new_version: u32,
}

// =============================================================================
// Vault methods
// =============================================================================

impl Client {
    /// List vault secrets.
    pub async fn vault_list(&self) -> ClientResult<VaultListResponse> {
        self.get("/_admin/vault/secrets").await
    }

    /// Get a vault secret.
    pub async fn vault_get(&self, key: &str) -> ClientResult<VaultGetResponse> {
        self.get(&format!("/_admin/vault/secrets/{}", urlencoding::encode(key)))
            .await
    }

    /// Set a vault secret.
    pub async fn vault_set(&self, key: &str, value: &str, is_base64: bool) -> ClientResult<()> {
        let request = VaultSetRequest {
            value: value.to_string(),
            is_base64,
        };
        let _: serde_json::Value = self.put(
            &format!("/_admin/vault/secrets/{}", urlencoding::encode(key)),
            &request,
        )
        .await?;
        Ok(())
    }

    /// Delete a vault secret.
    pub async fn vault_delete(&self, key: &str) -> ClientResult<()> {
        let _: serde_json::Value = self.delete(&format!(
            "/_admin/vault/secrets/{}",
            urlencoding::encode(key)
        ))
        .await?;
        Ok(())
    }

    /// Rotate a vault transit key.
    pub async fn vault_rotate(&self, key_name: &str) -> ClientResult<VaultRotateResponse> {
        #[derive(Serialize)]
        struct RotateRequest<'a> {
            key_name: &'a str,
        }

        self.post("/_admin/vault/rotate", &RotateRequest { key_name })
            .await
    }
}
