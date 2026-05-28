//! Connect capability client (`/connect/v1/*`).

use crate::error::ClientResult;
use crate::http::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Types
// ============================================================================

/// Connector descriptor from the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDescriptor {
    pub type_id: String,
    pub display_name: String,
    pub version: String,
    pub runtime: String,
    pub auth: AuthDescriptor,
    pub streams: Vec<StreamDescriptor>,
    pub actions: Vec<ActionDescriptor>,
    pub webhooks: Vec<WebhookDescriptor>,
    #[serde(default)]
    pub capabilities: ConnectorCapabilities,
    #[serde(default)]
    pub rate_limits: Option<RateLimitDescriptor>,
    #[serde(default)]
    pub doc_url: Option<String>,
}

/// Authentication descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDescriptor {
    pub kind: serde_json::Value,
    pub fields: Vec<AuthField>,
    #[serde(default)]
    pub test: Option<TestCallDescriptor>,
}

/// Authentication field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthField {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Test call descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCallDescriptor {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub success_codes: Vec<u16>,
}

/// Stream descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDescriptor {
    pub name: String,
    pub json_schema: serde_json::Value,
    pub supported_modes: Vec<String>,
    #[serde(default)]
    pub cursor_field: Option<Vec<String>>,
    #[serde(default)]
    pub primary_key: Option<Vec<Vec<String>>>,
    #[serde(default)]
    pub supports_outbound: bool,
    #[serde(default)]
    pub source_defined: bool,
}

/// Action descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDescriptor {
    pub name: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub side_effects: String,
    pub dry_run: String,
    #[serde(default)]
    pub idempotency: Option<IdempotencyHint>,
}

/// Idempotency hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyHint {
    pub key_path: String,
    pub ttl_seconds: u64,
}

/// Webhook descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDescriptor {
    pub name: String,
    pub verification: serde_json::Value,
    pub event_types: Vec<String>,
    #[serde(default)]
    pub replay_window_seconds: u64,
    #[serde(default)]
    pub setup_instructions: String,
}

/// Connector capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectorCapabilities {
    #[serde(default)]
    pub sandbox_mode: bool,
    #[serde(default)]
    pub vendor_test_mode: bool,
    #[serde(default)]
    pub cdc: bool,
    #[serde(default)]
    pub incremental: bool,
    #[serde(default)]
    pub schema_discovery: bool,
}

/// Rate limit descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitDescriptor {
    #[serde(default)]
    pub requests_per_second: Option<u32>,
    #[serde(default)]
    pub requests_per_minute: Option<u32>,
    #[serde(default)]
    pub requests_per_hour: Option<u32>,
    #[serde(default)]
    pub requests_per_day: Option<u32>,
    #[serde(default)]
    pub concurrent_requests: Option<u32>,
}

/// Connector instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: Uuid,
    pub connector_type: String,
    pub name: String,
    #[serde(default)]
    pub config: serde_json::Value,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Create instance request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInstanceRequest {
    pub connector_type: String,
    pub name: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Update instance request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInstanceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

/// Connection check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
}

/// Action invocation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeActionRequest {
    pub input: serde_json::Value,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// Action result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub output: serde_json::Value,
    #[serde(default)]
    pub dry_run: bool,
}

/// Webhook receiver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receiver {
    pub id: Uuid,
    pub name: String,
    pub target: ReceiverTarget,
    pub status: String,
    #[serde(default)]
    pub filter_expression: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Receiver target configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReceiverTarget {
    Job { name: String },
    Stream { connection: String },
    Action { instance: String, action: String },
    Function { name: String },
}

/// Create receiver request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReceiverRequest {
    pub name: String,
    pub target: ReceiverTarget,
    #[serde(default)]
    pub filter_expression: Option<String>,
}

/// Receiver with token (returned on create).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiverWithToken {
    #[serde(flatten)]
    pub receiver: Receiver,
    pub token: String,
}

/// Rotate token response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateTokenResponse {
    pub new_token: String,
    pub old_token_expires_at: chrono::DateTime<chrono::Utc>,
}

/// Schema drift event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEvent {
    pub id: Uuid,
    pub connection_id: Uuid,
    #[serde(default)]
    pub connection_name: Option<String>,
    pub stream_name: String,
    pub drift_type: String,
    pub severity: String,
    pub status: String,
    pub details: serde_json::Value,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub decided_by: Option<Uuid>,
    #[serde(default)]
    pub decided_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub decision_reason: Option<String>,
}

// ============================================================================
// Client Implementation
// ============================================================================

impl Client {
    // ----------------------------------------
    // Catalog
    // ----------------------------------------

    /// List available connectors in the catalog.
    pub async fn connect_catalog_list(&self) -> ClientResult<Vec<ConnectorDescriptor>> {
        self.get("/connect/v1/catalog").await
    }

    /// Get connector details from the catalog.
    pub async fn connect_catalog_get(&self, connector_type: &str) -> ClientResult<ConnectorDescriptor> {
        self.get(&format!("/connect/v1/catalog/{}", connector_type)).await
    }

    // ----------------------------------------
    // Instances
    // ----------------------------------------

    /// List connector instances.
    pub async fn connect_instances_list(&self) -> ClientResult<Vec<Instance>> {
        self.get("/connect/v1/instances").await
    }

    /// Create a connector instance.
    pub async fn connect_instances_create(&self, request: CreateInstanceRequest) -> ClientResult<Instance> {
        self.post("/connect/v1/instances", &request).await
    }

    /// Get instance details.
    pub async fn connect_instances_get(&self, instance_id: Uuid) -> ClientResult<Instance> {
        self.get(&format!("/connect/v1/instances/{}", instance_id)).await
    }

    /// Update an instance.
    pub async fn connect_instances_update(
        &self,
        instance_id: Uuid,
        request: UpdateInstanceRequest,
    ) -> ClientResult<Instance> {
        self.patch(&format!("/connect/v1/instances/{}", instance_id), &request)
            .await
    }

    /// Delete an instance.
    pub async fn connect_instances_delete(&self, instance_id: Uuid) -> ClientResult<()> {
        self.delete(&format!("/connect/v1/instances/{}", instance_id))
            .await
    }

    /// Test instance credentials.
    pub async fn connect_instances_check(&self, instance_id: Uuid) -> ClientResult<ConnectionStatus> {
        self.post(&format!("/connect/v1/instances/{}/check", instance_id), &())
            .await
    }

    /// Set credentials for an instance.
    pub async fn connect_instances_credentials(
        &self,
        instance_id: Uuid,
        credentials: serde_json::Value,
    ) -> ClientResult<()> {
        self.post(
            &format!("/connect/v1/instances/{}/credentials", instance_id),
            &credentials,
        )
        .await
    }

    // ----------------------------------------
    // Actions
    // ----------------------------------------

    /// Invoke an action on a connector instance.
    pub async fn connect_action_invoke(
        &self,
        instance_id: Uuid,
        action: &str,
        request: InvokeActionRequest,
    ) -> ClientResult<ActionResult> {
        self.post(
            &format!("/connect/v1/instances/{}/actions/{}", instance_id, action),
            &request,
        )
        .await
    }

    // ----------------------------------------
    // Receivers
    // ----------------------------------------

    /// List webhook receivers.
    pub async fn connect_receivers_list(&self) -> ClientResult<Vec<Receiver>> {
        self.get("/connect/v1/receivers").await
    }

    /// Create a webhook receiver.
    pub async fn connect_receivers_create(
        &self,
        request: CreateReceiverRequest,
    ) -> ClientResult<ReceiverWithToken> {
        self.post("/connect/v1/receivers", &request).await
    }

    /// Get receiver details.
    pub async fn connect_receivers_get(&self, receiver_id: Uuid) -> ClientResult<Receiver> {
        self.get(&format!("/connect/v1/receivers/{}", receiver_id)).await
    }

    /// Delete a receiver.
    pub async fn connect_receivers_delete(&self, receiver_id: Uuid) -> ClientResult<()> {
        self.delete(&format!("/connect/v1/receivers/{}", receiver_id))
            .await
    }

    /// Rotate receiver token.
    pub async fn connect_receivers_rotate(
        &self,
        receiver_id: Uuid,
        grace_seconds: u64,
    ) -> ClientResult<RotateTokenResponse> {
        #[derive(Serialize)]
        struct RotateRequest {
            grace_seconds: u64,
        }
        self.post(
            &format!("/connect/v1/receivers/{}/rotate", receiver_id),
            &RotateRequest { grace_seconds },
        )
        .await
    }

    // ----------------------------------------
    // Drift
    // ----------------------------------------

    /// List schema drift events.
    pub async fn connect_drift_list(
        &self,
        connection: Option<&str>,
        all: bool,
    ) -> ClientResult<Vec<DriftEvent>> {
        let mut url = "/connect/v1/drift".to_string();
        let mut params = vec![];
        if let Some(conn) = connection {
            params.push(format!("connection={}", conn));
        }
        if !all {
            params.push("status=pending".to_string());
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        #[derive(Deserialize)]
        struct ListResponse {
            events: Vec<DriftEvent>,
        }
        let resp: ListResponse = self.get(&url).await?;
        Ok(resp.events)
    }

    /// Approve a drift event.
    pub async fn connect_drift_approve(
        &self,
        drift_id: Uuid,
        reason: Option<&str>,
    ) -> ClientResult<DriftEvent> {
        #[derive(Serialize)]
        struct ApproveRequest {
            reason: Option<String>,
        }
        self.post(
            &format!("/connect/v1/drift/{}/approve", drift_id),
            &ApproveRequest {
                reason: reason.map(String::from),
            },
        )
        .await
    }

    /// Reject a drift event.
    pub async fn connect_drift_reject(
        &self,
        drift_id: Uuid,
        reason: Option<&str>,
    ) -> ClientResult<DriftEvent> {
        #[derive(Serialize)]
        struct RejectRequest {
            reason: Option<String>,
        }
        self.post(
            &format!("/connect/v1/drift/{}/reject", drift_id),
            &RejectRequest {
                reason: reason.map(String::from),
            },
        )
        .await
    }
}
