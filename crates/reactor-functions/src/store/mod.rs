//! Functions store abstractions.
//!
//! Provides traits and implementations for:
//! - FunctionsStore: Function and deployment metadata in PostgreSQL
//! - FunctionsTx: Transactional operations

mod postgres;

pub use postgres::PgFunctionsStore;

use crate::error::FunctionsError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Function ID type alias.
pub type FunctionId = Uuid;

/// Deployment ID type alias.
pub type DeploymentId = Uuid;

/// Invocation ID type alias.
pub type InvocationId = Uuid;

/// Policy ID type alias.
pub type PolicyId = Uuid;

/// Audit event ID type alias.
pub type AuditEventId = Uuid;

/// Function record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Function {
    /// Unique function ID.
    pub id: FunctionId,
    /// Organization that owns this function.
    pub org_id: Uuid,
    /// Function name (unique within org).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Runtime type: 'wasm', 'bun', or 'lambda'.
    pub runtime: String,
    /// Currently deployed version (null until first promote).
    pub current_deployment_id: Option<DeploymentId>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a function.
#[derive(Debug, Clone)]
pub struct FunctionCreate {
    /// Organization ID.
    pub org_id: Uuid,
    /// Function name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Runtime type.
    pub runtime: String,
}

/// Deployment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatus {
    /// Bundle uploaded, awaiting runtime materialization.
    Pending,
    /// Ready to receive traffic.
    Ready,
    /// Materialization failed.
    Failed,
    /// Resources cleaned up.
    Destroyed,
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentStatus::Pending => write!(f, "pending"),
            DeploymentStatus::Ready => write!(f, "ready"),
            DeploymentStatus::Failed => write!(f, "failed"),
            DeploymentStatus::Destroyed => write!(f, "destroyed"),
        }
    }
}

impl std::str::FromStr for DeploymentStatus {
    type Err = FunctionsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(DeploymentStatus::Pending),
            "ready" => Ok(DeploymentStatus::Ready),
            "failed" => Ok(DeploymentStatus::Failed),
            "destroyed" => Ok(DeploymentStatus::Destroyed),
            _ => Err(FunctionsError::Internal(format!(
                "invalid deployment status: {}",
                s
            ))),
        }
    }
}

/// Deployment record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Deployment {
    /// Unique deployment ID.
    pub id: DeploymentId,
    /// Function this deployment belongs to.
    pub function_id: FunctionId,
    /// Monotonically increasing version number.
    pub version: i64,
    /// Storage bucket (always '_reactor_functions').
    pub bundle_bucket: String,
    /// Object key in storage.
    pub bundle_object_key: String,
    /// SHA256 hash of the bundle.
    pub bundle_sha256: Vec<u8>,
    /// Bundle size in bytes.
    pub bundle_size: i64,
    /// Full manifest JSON.
    pub manifest_json: serde_json::Value,
    /// Current status.
    pub status: String,
    /// Error message if failed.
    pub status_detail: Option<String>,
    /// Adapter-specific reference (e.g., Lambda ARN).
    pub runtime_ref: Option<String>,
    /// When the deployment was created.
    pub deployed_at: DateTime<Utc>,
    /// User who created the deployment.
    pub deployed_by_user_id: Option<Uuid>,
}

/// Input for creating a deployment.
#[derive(Debug, Clone)]
pub struct DeploymentCreate {
    /// Function ID.
    pub function_id: FunctionId,
    /// Storage bucket.
    pub bundle_bucket: String,
    /// Object key.
    pub bundle_object_key: String,
    /// Bundle SHA256.
    pub bundle_sha256: Vec<u8>,
    /// Bundle size.
    pub bundle_size: i64,
    /// Manifest JSON.
    pub manifest_json: serde_json::Value,
    /// Deploying user ID.
    pub deployed_by_user_id: Option<Uuid>,
}

/// Environment variable record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnvVar {
    /// Function this env var belongs to.
    pub function_id: FunctionId,
    /// Variable key.
    pub key: String,
    /// Plaintext value (if not secret).
    pub value_plaintext: Option<String>,
    /// Encrypted value (if secret).
    pub value_encrypted: Option<Vec<u8>>,
    /// Whether this is a secret.
    pub is_secret: bool,
    /// Last update timestamp.
    pub last_updated_at: DateTime<Utc>,
}

/// Policy record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Policy {
    /// Unique policy ID.
    pub id: PolicyId,
    /// Function this policy belongs to.
    pub function_id: FunctionId,
    /// Policy name.
    pub name: String,
    /// Compiled policy expression as JSON.
    pub using_expr_json: Option<serde_json::Value>,
    /// Original policy text.
    pub raw_text: String,
    /// SHA256 hash of the policy text.
    pub sha256: Vec<u8>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Invocation record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Invocation {
    /// Unique invocation ID.
    pub id: InvocationId,
    /// Deployment that handled this invocation.
    pub deployment_id: DeploymentId,
    /// Function ID (denormalized).
    pub function_id: FunctionId,
    /// Organization ID.
    pub org_id: Uuid,
    /// User who made the request.
    pub actor_user_id: Option<Uuid>,
    /// API key that made the request.
    pub actor_apikey_id: Option<Uuid>,
    /// Request ID for tracing.
    pub request_id: String,
    /// HTTP method.
    pub method: String,
    /// Sub-path within the function.
    pub sub_path: String,
    /// HTTP status code.
    pub status_code: i32,
    /// Duration in milliseconds.
    pub duration_ms: i32,
    /// Whether this was a cold start.
    pub cold_start: bool,
    /// Request body size.
    pub bytes_in: i64,
    /// Response body size.
    pub bytes_out: i64,
    /// Platform error code if any.
    pub error_code: Option<String>,
    /// When the invocation started.
    pub started_at: DateTime<Utc>,
}

/// Input for recording an invocation.
#[derive(Debug, Clone)]
pub struct InvocationCreate {
    /// Deployment ID.
    pub deployment_id: DeploymentId,
    /// Function ID.
    pub function_id: FunctionId,
    /// Organization ID.
    pub org_id: Uuid,
    /// Actor user ID.
    pub actor_user_id: Option<Uuid>,
    /// Actor API key ID.
    pub actor_apikey_id: Option<Uuid>,
    /// Request ID.
    pub request_id: String,
    /// HTTP method.
    pub method: String,
    /// Sub-path.
    pub sub_path: String,
    /// Status code.
    pub status_code: i32,
    /// Duration in ms.
    pub duration_ms: i32,
    /// Cold start flag.
    pub cold_start: bool,
    /// Bytes in.
    pub bytes_in: i64,
    /// Bytes out.
    pub bytes_out: i64,
    /// Error code.
    pub error_code: Option<String>,
}

/// Audit event record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEvent {
    /// Unique event ID.
    pub id: AuditEventId,
    /// Event timestamp.
    pub ts: DateTime<Utc>,
    /// Actor user ID.
    pub actor_user_id: Option<Uuid>,
    /// Actor API key ID.
    pub actor_apikey_id: Option<Uuid>,
    /// Organization ID.
    pub org_id: Option<Uuid>,
    /// Function ID (if applicable).
    pub function_id: Option<FunctionId>,
    /// Deployment ID (if applicable).
    pub deployment_id: Option<DeploymentId>,
    /// Event type (e.g., 'function.create').
    pub event_type: String,
    /// Additional event details.
    pub details: serde_json::Value,
    /// Request ID for tracing.
    pub request_id: String,
}

/// Input for creating an audit event.
#[derive(Debug, Clone)]
pub struct AuditEventCreate {
    /// Actor user ID.
    pub actor_user_id: Option<Uuid>,
    /// Actor API key ID.
    pub actor_apikey_id: Option<Uuid>,
    /// Organization ID.
    pub org_id: Option<Uuid>,
    /// Function ID.
    pub function_id: Option<FunctionId>,
    /// Deployment ID.
    pub deployment_id: Option<DeploymentId>,
    /// Event type.
    pub event_type: String,
    /// Event details.
    pub details: serde_json::Value,
    /// Request ID.
    pub request_id: String,
}

/// Functions store trait.
#[async_trait]
pub trait FunctionsStore: Send + Sync {
    // Functions
    /// Create a new function.
    async fn create_function(&self, input: FunctionCreate) -> Result<Function, FunctionsError>;

    /// Get a function by ID.
    async fn get_function(&self, id: FunctionId) -> Result<Option<Function>, FunctionsError>;

    /// Get a function by org and name.
    async fn get_function_by_name(
        &self,
        org_id: Uuid,
        name: &str,
    ) -> Result<Option<Function>, FunctionsError>;

    /// List functions for an org.
    async fn list_functions(&self, org_id: Uuid) -> Result<Vec<Function>, FunctionsError>;

    /// Delete a function.
    async fn delete_function(&self, id: FunctionId) -> Result<bool, FunctionsError>;

    /// Update the current deployment for a function.
    async fn set_current_deployment(
        &self,
        function_id: FunctionId,
        deployment_id: Option<DeploymentId>,
    ) -> Result<(), FunctionsError>;

    // Deployments
    /// Create a new deployment.
    async fn create_deployment(&self, input: DeploymentCreate)
        -> Result<Deployment, FunctionsError>;

    /// Get a deployment by ID.
    async fn get_deployment(&self, id: DeploymentId) -> Result<Option<Deployment>, FunctionsError>;

    /// Get the current deployment for a function.
    async fn get_current_deployment(
        &self,
        function_id: FunctionId,
    ) -> Result<Option<Deployment>, FunctionsError>;

    /// List deployments for a function.
    async fn list_deployments(
        &self,
        function_id: FunctionId,
    ) -> Result<Vec<Deployment>, FunctionsError>;

    /// Update deployment status.
    async fn update_deployment_status(
        &self,
        id: DeploymentId,
        status: DeploymentStatus,
        status_detail: Option<String>,
        runtime_ref: Option<String>,
    ) -> Result<(), FunctionsError>;

    /// Get the next version number for a function.
    async fn next_deployment_version(
        &self,
        function_id: FunctionId,
    ) -> Result<i64, FunctionsError>;

    // Env
    /// Get all env vars for a function.
    async fn get_env(&self, function_id: FunctionId) -> Result<Vec<EnvVar>, FunctionsError>;

    /// Get a single env var.
    async fn get_env_var(
        &self,
        function_id: FunctionId,
        key: &str,
    ) -> Result<Option<EnvVar>, FunctionsError>;

    /// Upsert an env var.
    async fn upsert_env(
        &self,
        function_id: FunctionId,
        key: &str,
        value_plaintext: Option<String>,
        value_encrypted: Option<Vec<u8>>,
        is_secret: bool,
    ) -> Result<(), FunctionsError>;

    /// Delete an env var.
    async fn delete_env(&self, function_id: FunctionId, key: &str) -> Result<bool, FunctionsError>;

    // Policies
    /// Create a policy.
    async fn create_policy(
        &self,
        function_id: FunctionId,
        name: &str,
        using_expr_json: Option<serde_json::Value>,
        raw_text: &str,
        sha256: Vec<u8>,
    ) -> Result<Policy, FunctionsError>;

    /// Get all policies for a function.
    async fn get_policies(&self, function_id: FunctionId) -> Result<Vec<Policy>, FunctionsError>;

    /// Delete a policy.
    async fn delete_policy(&self, function_id: FunctionId, name: &str)
        -> Result<bool, FunctionsError>;

    // Invocations
    /// Record an invocation.
    async fn record_invocation(&self, input: InvocationCreate) -> Result<(), FunctionsError>;

    // Audit
    /// Record an audit event.
    async fn record_audit_event(&self, input: AuditEventCreate) -> Result<(), FunctionsError>;
}

/// Transaction trait for atomic operations.
#[async_trait]
pub trait FunctionsTx: Send + Sync {
    /// Run a closure within a transaction.
    async fn transaction<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'c> FnOnce(&'c mut sqlx::Transaction<'static, sqlx::Postgres>) -> futures::future::BoxFuture<'c, Result<T, E>>
            + Send,
        T: Send,
        E: From<sqlx::Error> + Send;
}
