//! Sandbox endpoints.
//!
//! Provides both action dry-run sandbox and stream sandbox functionality.

use crate::error::ConnectError;
use crate::service::ConnectService;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::ConnectStore;
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Action sandbox request.
#[derive(Debug, Deserialize)]
pub struct ActionSandboxRequest {
    /// Action input.
    pub input: serde_json::Value,
}

/// Action sandbox response.
#[derive(Debug, Serialize)]
pub struct ActionSandboxResponse {
    /// The request that would have been sent.
    pub would_have_sent: Option<WouldHaveSent>,
    /// Estimated cost (if available).
    pub estimated_cost: Option<serde_json::Value>,
    /// Validation result.
    pub validation: ValidationResult,
}

/// HTTP request that would have been sent.
#[derive(Debug, Serialize)]
pub struct WouldHaveSent {
    /// HTTP method.
    pub method: String,
    /// URL.
    pub url: String,
    /// Request body.
    pub body: Option<serde_json::Value>,
    /// Headers (sanitized).
    pub headers: std::collections::HashMap<String, String>,
}

/// Validation result.
#[derive(Debug, Serialize)]
pub struct ValidationResult {
    /// Whether validation passed.
    pub ok: bool,
    /// Validation errors.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,
}

/// Validation error.
#[derive(Debug, Serialize)]
pub struct ValidationError {
    /// JSON path to the error.
    pub path: String,
    /// Error message.
    pub message: String,
}

/// POST /connect/v1/instances/:name/actions/:action/sandbox
pub async fn action_sandbox<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path((name, action)): Path<(String, String)>,
    Json(req): Json<ActionSandboxRequest>,
) -> Result<Json<ActionSandboxResponse>, ConnectError> {
    let service = ConnectService::new(&state, &ctx);

    // Try to invoke in dry-run mode
    match service
        .invoke_action(&name, &action, req.input.clone(), None, true)
        .await
    {
        Ok(output) => {
            // Dry-run succeeded, extract the would-have-sent info
            Ok(Json(ActionSandboxResponse {
                would_have_sent: Some(WouldHaveSent {
                    method: "POST".to_string(), // Placeholder
                    url: format!("https://api.example.com/{}", action),
                    body: Some(req.input),
                    headers: std::collections::HashMap::new(),
                }),
                estimated_cost: None,
                validation: ValidationResult {
                    ok: true,
                    errors: vec![],
                },
            }))
        }
        Err(ConnectError::DryRunNotSupported(_)) => {
            // Return 422 with suggestion
            Err(ConnectError::DryRunNotSupported(action))
        }
        Err(e) => Err(e),
    }
}

// =============================================================================
// Stream Sandbox Endpoints
// =============================================================================

/// Stream sandbox metadata.
#[derive(Debug, Serialize)]
pub struct StreamSandboxInfo {
    /// Sandbox ID.
    pub id: Uuid,
    /// Connection ID.
    pub connection_id: Uuid,
    /// Schema name.
    pub schema_name: String,
    /// When the sandbox was created.
    pub created_at: DateTime<Utc>,
    /// When the sandbox expires.
    pub expires_at: DateTime<Utc>,
    /// Promote token (HMAC-signed, can be used once to promote).
    pub promote_token: String,
    /// Schema diff summary.
    pub diff: SandboxDiff,
    /// Stats.
    pub stats: SandboxStats,
}

/// Sandbox diff summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxDiff {
    /// Per-stream diffs.
    pub streams: std::collections::HashMap<String, StreamDiff>,
    /// Total rows added.
    pub total_rows_added: u64,
    /// Total columns added.
    pub total_columns_added: u64,
    /// Total type changes.
    pub total_type_changes: u64,
}

impl Default for SandboxDiff {
    fn default() -> Self {
        Self {
            streams: std::collections::HashMap::new(),
            total_rows_added: 0,
            total_columns_added: 0,
            total_type_changes: 0,
        }
    }
}

/// Per-stream diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDiff {
    /// Number of rows.
    pub row_count: u64,
    /// New columns.
    pub added_columns: Vec<String>,
    /// Removed columns.
    pub removed_columns: Vec<String>,
    /// Type changes.
    pub type_changes: Vec<TypeChange>,
}

/// Column type change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeChange {
    /// Column name.
    pub column: String,
    /// Old type.
    pub old_type: String,
    /// New type.
    pub new_type: String,
}

/// Sandbox stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxStats {
    /// Total records.
    pub total_records: u64,
    /// Total bytes.
    pub total_bytes: u64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// GET /connect/v1/connections/:name/sandbox
pub async fn get_sandbox<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<Json<Option<StreamSandboxInfo>>, ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name))?;

    // TODO: Look up sandbox schema from _reactor_connect.sandbox_schemas
    // For now, return None
    Ok(Json(None))
}

/// Promote sandbox request.
#[derive(Debug, Deserialize)]
pub struct PromoteSandboxRequest {
    /// The promote token from the sandbox info.
    pub promote_token: String,
}

/// Promote sandbox response.
#[derive(Debug, Serialize)]
pub struct PromoteSandboxResponse {
    /// Whether promotion succeeded.
    pub success: bool,
    /// Message.
    pub message: String,
    /// New table name (if promoted).
    pub table_name: Option<String>,
}

/// POST /connect/v1/connections/:name/sandbox/promote
pub async fn promote_sandbox<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
    Json(req): Json<PromoteSandboxRequest>,
) -> Result<Json<PromoteSandboxResponse>, ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name.clone()))?;

    // TODO: Verify promote_token (HMAC verification)
    // TODO: Move sandbox schema to production
    // TODO: Mark sandbox as promoted

    Ok(Json(PromoteSandboxResponse {
        success: true,
        message: format!("Sandbox for connection '{}' promoted", name),
        table_name: Some(format!("connect_{}", connection.name)),
    }))
}

/// DELETE /connect/v1/connections/:name/sandbox
pub async fn delete_sandbox<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<(), ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name))?;

    // TODO: Delete sandbox schema
    // DELETE FROM _reactor_connect.sandbox_schemas WHERE connection_id = $1
    // DROP SCHEMA IF EXISTS "_sandbox_..." CASCADE

    Ok(())
}
