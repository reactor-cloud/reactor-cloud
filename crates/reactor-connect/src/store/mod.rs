//! Connect data store.

mod postgres;

pub use postgres::PgConnectStore;

use crate::error::ConnectError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Organization ID type alias.
pub type OrgId = Uuid;
/// Instance ID type alias.
pub type InstanceId = Uuid;
/// Connection ID type alias.
pub type ConnectionId = Uuid;
/// Receiver ID type alias.
pub type ReceiverId = Uuid;
/// Sync run ID type alias.
pub type SyncRunId = Uuid;
/// Action invocation ID type alias.
pub type ActionInvocationId = Uuid;

/// A configured connector instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    /// Unique ID.
    pub id: InstanceId,
    /// Organization ID.
    pub org_id: OrgId,
    /// Connector type ID.
    pub type_id: String,
    /// User-defined name.
    pub name: String,
    /// Non-secret configuration.
    pub config_json: serde_json::Value,
    /// Vault reference for credentials.
    pub vault_ref: Option<String>,
    /// Credential state.
    pub credential_state: String,
    /// Credential error message.
    pub credential_error: Option<String>,
    /// Whether the instance is enabled.
    pub enabled: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// New instance to create.
#[derive(Debug, Clone)]
pub struct NewInstance {
    /// Connector type ID.
    pub type_id: String,
    /// User-defined name.
    pub name: String,
    /// Non-secret configuration.
    pub config_json: serde_json::Value,
}

/// A stream connection (source → destination).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Unique ID.
    pub id: ConnectionId,
    /// Organization ID.
    pub org_id: OrgId,
    /// User-defined name.
    pub name: String,
    /// Source instance ID (if instance source).
    pub source_instance_id: Option<InstanceId>,
    /// Source kind.
    pub source_kind: String,
    /// Source configuration.
    pub source_config_json: serde_json::Value,
    /// Destination instance ID (if instance destination).
    pub dest_instance_id: Option<InstanceId>,
    /// Destination kind.
    pub dest_kind: String,
    /// Destination configuration.
    pub dest_config_json: serde_json::Value,
    /// Schedule kind.
    pub schedule_kind: String,
    /// Schedule configuration.
    pub schedule_config_json: serde_json::Value,
    /// Options.
    pub options_json: serde_json::Value,
    /// Whether the connection is enabled.
    pub enabled: bool,
    /// Direction.
    pub direction: String,
    /// Last sync timestamp.
    pub last_sync_at: Option<DateTime<Utc>>,
    /// Reactor-jobs job name.
    pub job_name: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// New connection to create.
#[derive(Debug, Clone)]
pub struct NewConnection {
    /// User-defined name.
    pub name: String,
    /// Source instance ID.
    pub source_instance_id: Option<InstanceId>,
    /// Source kind.
    pub source_kind: String,
    /// Source configuration.
    pub source_config_json: serde_json::Value,
    /// Destination instance ID.
    pub dest_instance_id: Option<InstanceId>,
    /// Destination kind.
    pub dest_kind: String,
    /// Destination configuration.
    pub dest_config_json: serde_json::Value,
    /// Schedule kind.
    pub schedule_kind: String,
    /// Schedule configuration.
    pub schedule_config_json: serde_json::Value,
    /// Options.
    pub options_json: serde_json::Value,
    /// Direction.
    pub direction: String,
}

/// A webhook receiver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receiver {
    /// Unique ID.
    pub id: ReceiverId,
    /// Instance ID.
    pub instance_id: InstanceId,
    /// Organization ID.
    pub org_id: OrgId,
    /// Webhook name from descriptor.
    pub webhook_name: String,
    /// Ingress token.
    pub token: String,
    /// Dispatch kind.
    pub dispatch_kind: String,
    /// Dispatch configuration.
    pub dispatch_config_json: serde_json::Value,
    /// Whether the receiver is enabled.
    pub enabled: bool,
    /// Last received timestamp.
    pub last_received_at: Option<DateTime<Utc>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// New receiver to create.
#[derive(Debug, Clone)]
pub struct NewReceiver {
    /// Instance ID.
    pub instance_id: InstanceId,
    /// Webhook name from descriptor.
    pub webhook_name: String,
    /// Dispatch kind.
    pub dispatch_kind: String,
    /// Dispatch configuration.
    pub dispatch_config_json: serde_json::Value,
}

/// State bundle for a connection stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateBundle {
    /// Connection ID.
    pub connection_id: ConnectionId,
    /// Stream name.
    pub stream_name: String,
    /// State JSON.
    pub state_json: serde_json::Value,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Sync run record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRunRecord {
    /// Unique ID.
    pub id: SyncRunId,
    /// Connection ID.
    pub connection_id: ConnectionId,
    /// Organization ID.
    pub org_id: OrgId,
    /// Reactor-jobs run ID.
    pub jobs_run_id: Option<Uuid>,
    /// Status.
    pub status: String,
    /// Records read per stream.
    pub records_read: serde_json::Value,
    /// Records written per stream.
    pub records_written: serde_json::Value,
    /// Error code.
    pub error_code: Option<String>,
    /// Error message.
    pub error_message: Option<String>,
    /// Suggested fix.
    pub error_suggested_fix: Option<String>,
    /// Start timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Finish timestamp.
    pub finished_at: Option<DateTime<Utc>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Action invocation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInvocationRecord {
    /// Unique ID.
    pub id: ActionInvocationId,
    /// Instance ID.
    pub instance_id: InstanceId,
    /// Organization ID.
    pub org_id: OrgId,
    /// Action name.
    pub action_name: String,
    /// Input hash for dedup.
    pub input_hash: Option<Vec<u8>>,
    /// Idempotency key.
    pub idempotency_key: Option<String>,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// Status.
    pub status: String,
    /// Duration in milliseconds.
    pub duration_ms: Option<i32>,
    /// Error code.
    pub error_code: Option<String>,
    /// Error message.
    pub error_message: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Connect store trait.
#[async_trait]
pub trait ConnectStore: Send + Sync + Clone + 'static {
    /// Transaction type.
    type Tx<'a>: ConnectTx
    where
        Self: 'a;

    /// Begin a transaction.
    async fn begin(&self) -> Result<Self::Tx<'_>, ConnectError>;

    // Instances
    /// Create an instance.
    async fn create_instance(
        &self,
        org_id: &OrgId,
        instance: &NewInstance,
    ) -> Result<Instance, ConnectError>;

    /// Get an instance by name.
    async fn get_instance(
        &self,
        org_id: &OrgId,
        name: &str,
    ) -> Result<Option<Instance>, ConnectError>;

    /// Get an instance by ID.
    async fn get_instance_by_id(&self, id: &InstanceId) -> Result<Option<Instance>, ConnectError>;

    /// List instances for an org.
    async fn list_instances(&self, org_id: &OrgId) -> Result<Vec<Instance>, ConnectError>;

    /// Update instance credential state.
    async fn update_instance_credentials(
        &self,
        id: &InstanceId,
        vault_ref: &str,
        state: &str,
        error: Option<&str>,
    ) -> Result<(), ConnectError>;

    /// Delete an instance.
    async fn delete_instance(&self, id: &InstanceId) -> Result<(), ConnectError>;

    // Connections
    /// Create a connection.
    async fn create_connection(
        &self,
        org_id: &OrgId,
        conn: &NewConnection,
    ) -> Result<Connection, ConnectError>;

    /// Get a connection by name.
    async fn get_connection(
        &self,
        org_id: &OrgId,
        name: &str,
    ) -> Result<Option<Connection>, ConnectError>;

    /// Get a connection by ID.
    async fn get_connection_by_id(&self, id: &ConnectionId) -> Result<Option<Connection>, ConnectError>;

    /// List connections for an org.
    async fn list_connections(&self, org_id: &OrgId) -> Result<Vec<Connection>, ConnectError>;

    /// Set connection enabled state.
    async fn set_connection_enabled(
        &self,
        id: &ConnectionId,
        enabled: bool,
    ) -> Result<(), ConnectError>;

    /// Delete a connection.
    async fn delete_connection(&self, id: &ConnectionId) -> Result<(), ConnectError>;

    // Receivers
    /// Create a receiver.
    async fn create_receiver(
        &self,
        org_id: &OrgId,
        receiver: &NewReceiver,
    ) -> Result<Receiver, ConnectError>;

    /// Get a receiver by token.
    async fn get_receiver_by_token(&self, token: &str) -> Result<Option<Receiver>, ConnectError>;

    /// List receivers for an instance.
    async fn list_receivers(&self, instance_id: &InstanceId) -> Result<Vec<Receiver>, ConnectError>;

    /// Delete a receiver.
    async fn delete_receiver(&self, id: &ReceiverId) -> Result<(), ConnectError>;

    // State
    /// Get connection state for a stream.
    async fn get_state(
        &self,
        connection_id: &ConnectionId,
        stream_name: &str,
    ) -> Result<Option<StateBundle>, ConnectError>;

    /// Put connection state for a stream.
    async fn put_state(
        &self,
        connection_id: &ConnectionId,
        stream_name: &str,
        state: &serde_json::Value,
    ) -> Result<(), ConnectError>;

    // Runs
    /// Record a sync run.
    async fn record_run(&self, run: &SyncRunRecord) -> Result<(), ConnectError>;

    /// List runs for a connection.
    async fn list_runs(
        &self,
        connection_id: &ConnectionId,
        limit: u32,
    ) -> Result<Vec<SyncRunRecord>, ConnectError>;

    // Action invocations
    /// Record an action invocation.
    async fn record_invocation(&self, inv: &ActionInvocationRecord) -> Result<(), ConnectError>;

    // Audit
    /// Write an audit event.
    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), ConnectError>;
}

/// Connect transaction trait.
#[async_trait]
pub trait ConnectTx: Send {
    /// Execute a raw SQL statement.
    async fn execute_raw(&mut self, sql: &str, params: &[&str]) -> Result<u64, ConnectError>;

    /// Commit the transaction.
    async fn commit(self) -> Result<(), ConnectError>;

    /// Rollback the transaction.
    async fn rollback(self) -> Result<(), ConnectError>;
}

/// Audit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Event ID.
    pub id: Uuid,
    /// Timestamp.
    pub ts: DateTime<Utc>,
    /// Actor user ID.
    pub actor_user_id: Option<Uuid>,
    /// Actor API key ID.
    pub actor_apikey_id: Option<Uuid>,
    /// Organization ID.
    pub org_id: Option<OrgId>,
    /// Instance ID.
    pub instance_id: Option<InstanceId>,
    /// Connection ID.
    pub connection_id: Option<ConnectionId>,
    /// Receiver ID.
    pub receiver_id: Option<ReceiverId>,
    /// Event type.
    pub event_type: String,
    /// Event details.
    pub details: serde_json::Value,
    /// Request ID.
    pub request_id: String,
}
