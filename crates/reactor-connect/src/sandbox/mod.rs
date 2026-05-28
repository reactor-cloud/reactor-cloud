//! Sandbox execution for safe testing.
//!
//! Provides:
//! - Inbound sandbox: ephemeral schema for testing data ingestion
//! - Outbound sandbox: delivers to vendor test environments via descriptor-declared test creds

use crate::error::ConnectError;
use crate::state::ConnectState;
use crate::store::{ConnectStore, ConnectionId};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::watch;

/// Sandbox manager.
pub struct SandboxManager<S: ConnectStore> {
    #[allow(dead_code)]
    store: S,
    ttl: Duration,
}

impl<S: ConnectStore> SandboxManager<S> {
    /// Create a new sandbox manager.
    pub fn new(store: S, ttl: Duration) -> Self {
        Self { store, ttl }
    }

    /// Create an ephemeral sandbox schema.
    pub async fn create_sandbox_schema(
        &self,
        _connection_id: &ConnectionId,
    ) -> Result<SandboxSchema, ConnectError> {
        let schema_name = format!("_sandbox_{}", uuid::Uuid::new_v4().simple());

        // TODO: Create schema via reactor-data admin endpoint

        Ok(SandboxSchema {
            name: schema_name,
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::from_std(self.ttl).unwrap(),
        })
    }

    /// Generate a promote token for a sandbox run.
    pub fn generate_promote_token(
        &self,
        sandbox_run_id: &uuid::Uuid,
        connection_id: &ConnectionId,
        secret: &[u8],
    ) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let exp = chrono::Utc::now() + chrono::Duration::hours(1);
        let payload = format!("{}:{}:{}", sandbox_run_id, connection_id, exp.timestamp());

        let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC can take any size key");
        mac.update(payload.as_bytes());
        let signature = mac.finalize();

        base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            signature.into_bytes(),
        )
    }

    /// Verify a promote token.
    pub fn verify_promote_token(
        &self,
        token: &str,
        sandbox_run_id: &uuid::Uuid,
        connection_id: &ConnectionId,
        secret: &[u8],
    ) -> bool {
        // Regenerate and compare
        let expected = self.generate_promote_token(sandbox_run_id, connection_id, secret);
        token == expected
    }
}

/// Start cleanup worker for expired sandboxes with shutdown coordination.
///
/// This worker periodically checks for expired sandbox schemas and drops them.
pub async fn start_cleanup_worker_with_shutdown<S: ConnectStore>(
    _state: ConnectState<S>,
    mut shutdown: watch::Receiver<bool>,
    ttl: Duration,
) {
    // Check every 5 minutes or half the TTL, whichever is smaller
    let check_interval = std::cmp::min(Duration::from_secs(300), ttl / 2);
    let mut interval = tokio::time::interval(check_interval);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // TODO: Query expired sandboxes and drop schemas
                tracing::debug!("Sandbox cleanup check (placeholder)");
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("Sandbox cleanup worker shutting down");
                    break;
                }
            }
        }
    }
}

/// Sandbox schema info.
#[derive(Debug)]
pub struct SandboxSchema {
    /// Schema name.
    pub name: String,
    /// Creation time.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Expiration time.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// Outbound Sandbox Support
// ============================================================================

/// Sandbox mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    /// Inbound sandbox: writes to ephemeral schema.
    Inbound,
    /// Outbound sandbox: uses vendor test environment.
    Outbound,
    /// Bidirectional sandbox: both inbound and outbound are sandboxed.
    Bidirectional,
}

/// Test credentials declared in a connector descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCredentials {
    /// Credential key (e.g., "sandbox_api_key").
    pub key: String,
    /// Environment variable name to look up.
    pub env_var: Option<String>,
    /// Description for documentation.
    pub description: Option<String>,
    /// Whether this is a sandbox/test environment URL.
    pub is_test_url: bool,
}

/// Outbound sandbox configuration from connector descriptor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutboundSandboxSpec {
    /// Test credentials to use.
    #[serde(default)]
    pub test_credentials: Vec<TestCredentials>,
    /// Test environment base URL (if different).
    pub test_base_url: Option<String>,
    /// Documentation URL for setting up sandbox.
    pub setup_docs_url: Option<String>,
    /// Whether the connector supports native sandbox mode.
    #[serde(default)]
    pub native_sandbox: bool,
}

/// Build test credentials from environment and descriptor.
pub fn resolve_test_credentials(
    spec: &OutboundSandboxSpec,
    env_credentials: &serde_json::Value,
) -> Result<serde_json::Value, ConnectError> {
    let mut resolved = serde_json::Map::new();

    for cred in &spec.test_credentials {
        let value = if let Some(env_var) = &cred.env_var {
            // Look up from environment
            std::env::var(env_var).ok().map(serde_json::Value::String)
        } else {
            // Look up from provided credentials
            env_credentials.get(&cred.key).cloned()
        };

        if let Some(v) = value {
            resolved.insert(cred.key.clone(), v);
        }
    }

    // Add test base URL if specified
    if let Some(url) = &spec.test_base_url {
        resolved.insert("base_url".to_string(), serde_json::Value::String(url.clone()));
    }

    Ok(serde_json::Value::Object(resolved))
}
