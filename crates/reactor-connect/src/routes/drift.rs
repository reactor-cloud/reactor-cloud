//! Schema drift management endpoints.

use crate::error::ConnectError;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::ConnectStore;
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Drift event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEvent {
    /// Event ID.
    pub id: Uuid,
    /// Connection ID.
    pub connection_id: Uuid,
    /// Stream name.
    pub stream_name: String,
    /// Drift type.
    pub drift_type: DriftType,
    /// Severity.
    pub severity: DriftSeverity,
    /// Change details.
    pub details: DriftDetails,
    /// Status.
    pub status: DriftStatus,
    /// When the drift was detected.
    pub detected_at: DateTime<Utc>,
    /// Who approved/rejected.
    pub decided_by: Option<Uuid>,
    /// When the decision was made.
    pub decided_at: Option<DateTime<Utc>>,
    /// Decision reason.
    pub decision_reason: Option<String>,
}

/// Drift type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftType {
    ColumnAdded,
    ColumnRemoved,
    TypeChanged,
    PrimaryKeyChanged,
    StreamAdded,
    StreamRemoved,
}

/// Drift severity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    Info,
    Warning,
    Breaking,
}

/// Drift status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    Pending,
    Approved,
    Rejected,
}

/// Drift details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetails {
    /// Column name (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_name: Option<String>,
    /// Old type (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_type: Option<String>,
    /// New type (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_type: Option<String>,
    /// Additional context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

/// List drift events query.
#[derive(Debug, Deserialize)]
pub struct ListDriftQuery {
    /// Filter by connection name.
    pub connection: Option<String>,
    /// Filter by status.
    pub status: Option<DriftStatus>,
    /// Limit.
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

/// List drift response.
#[derive(Debug, Serialize)]
pub struct ListDriftResponse {
    /// Drift events.
    pub events: Vec<DriftEvent>,
    /// Total count of pending events.
    pub pending_count: i64,
}

/// GET /connect/v1/drift
pub async fn list_drift<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Query(query): Query<ListDriftQuery>,
) -> Result<Json<ListDriftResponse>, ConnectError> {
    // TODO: Query connection_drift_events table
    // For now return empty
    Ok(Json(ListDriftResponse {
        events: vec![],
        pending_count: 0,
    }))
}

/// Approve drift request.
#[derive(Debug, Deserialize)]
pub struct ApproveDriftRequest {
    /// Reason for approval.
    pub reason: Option<String>,
}

/// POST /connect/v1/drift/:id/approve
pub async fn approve_drift<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveDriftRequest>,
) -> Result<Json<DriftEvent>, ConnectError> {
    // TODO: Update connection_drift_events table
    // SET status = 'approved', decided_by = ctx.user_id, decided_at = now(), decision_reason = req.reason
    Err(ConnectError::DriftEventNotFound(id.to_string()))
}

/// Reject drift request.
#[derive(Debug, Deserialize)]
pub struct RejectDriftRequest {
    /// Reason for rejection.
    pub reason: Option<String>,
}

/// POST /connect/v1/drift/:id/reject
pub async fn reject_drift<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(id): Path<Uuid>,
    Json(req): Json<RejectDriftRequest>,
) -> Result<Json<DriftEvent>, ConnectError> {
    // TODO: Update connection_drift_events table
    // SET status = 'rejected', decided_by = ctx.user_id, decided_at = now(), decision_reason = req.reason
    Err(ConnectError::DriftEventNotFound(id.to_string()))
}
