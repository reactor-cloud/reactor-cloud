//! Connection pairs management endpoints.
//!
//! Manages bidirectional sync relationships between connections.

use crate::error::ConnectError;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::ConnectStore;
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Connection pair for bidirectional sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPair {
    /// Pair ID.
    pub id: Uuid,
    /// Pair name.
    pub name: String,
    /// Connection A ID.
    pub connection_a_id: Uuid,
    /// Connection A name.
    pub connection_a_name: Option<String>,
    /// Connection B ID.
    pub connection_b_id: Uuid,
    /// Connection B name.
    pub connection_b_name: Option<String>,
    /// Conflict policy ID.
    pub conflict_policy_id: Option<Uuid>,
    /// Loop protection enabled.
    pub loop_protection_enabled: bool,
    /// Loop protection window in seconds.
    pub loop_protection_window_secs: i64,
    /// Whether this pair is enabled.
    pub enabled: bool,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Updated at.
    pub updated_at: DateTime<Utc>,
}

/// Create pair request.
#[derive(Debug, Deserialize)]
pub struct CreatePairRequest {
    /// Pair name.
    pub name: String,
    /// Connection A ID.
    pub connection_a_id: Uuid,
    /// Connection B ID.
    pub connection_b_id: Uuid,
    /// Conflict policy ID (optional).
    pub conflict_policy_id: Option<Uuid>,
    /// Loop protection enabled (default true).
    #[serde(default = "default_true")]
    pub loop_protection_enabled: bool,
    /// Loop protection window in seconds (default 300 = 5 minutes).
    #[serde(default = "default_window")]
    pub loop_protection_window_secs: i64,
}

fn default_true() -> bool {
    true
}

fn default_window() -> i64 {
    300
}

/// Update pair request.
#[derive(Debug, Deserialize)]
pub struct UpdatePairRequest {
    /// Conflict policy ID.
    pub conflict_policy_id: Option<Uuid>,
    /// Loop protection enabled.
    pub loop_protection_enabled: Option<bool>,
    /// Loop protection window in seconds.
    pub loop_protection_window_secs: Option<i64>,
    /// Whether this pair is enabled.
    pub enabled: Option<bool>,
}

/// GET /connect/v1/pairs
pub async fn list<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
) -> Result<Json<Vec<ConnectionPair>>, ConnectError> {
    // TODO: Query _reactor_connect.connection_pairs
    Ok(Json(vec![]))
}

/// POST /connect/v1/pairs
pub async fn create<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Json(req): Json<CreatePairRequest>,
) -> Result<Json<ConnectionPair>, ConnectError> {
    // Validate connections are different
    if req.connection_a_id == req.connection_b_id {
        return Err(ConnectError::InvalidInput(
            "Connection A and B must be different".to_string(),
        ));
    }

    let now = Utc::now();
    let pair = ConnectionPair {
        id: Uuid::new_v4(),
        name: req.name,
        connection_a_id: req.connection_a_id,
        connection_a_name: None,
        connection_b_id: req.connection_b_id,
        connection_b_name: None,
        conflict_policy_id: req.conflict_policy_id,
        loop_protection_enabled: req.loop_protection_enabled,
        loop_protection_window_secs: req.loop_protection_window_secs,
        enabled: true,
        created_at: now,
        updated_at: now,
    };

    // TODO: Insert into _reactor_connect.connection_pairs
    // TODO: Update both connections with pair_id

    Ok(Json(pair))
}

/// GET /connect/v1/pairs/:id
pub async fn get<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConnectionPair>, ConnectError> {
    // TODO: Query _reactor_connect.connection_pairs WHERE id = $1
    Err(ConnectError::InvalidInput(format!("Pair {} not found", id)))
}

/// PATCH /connect/v1/pairs/:id
pub async fn update<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePairRequest>,
) -> Result<Json<ConnectionPair>, ConnectError> {
    // TODO: Update _reactor_connect.connection_pairs
    Err(ConnectError::InvalidInput(format!("Pair {} not found", id)))
}

/// DELETE /connect/v1/pairs/:id
pub async fn delete<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(id): Path<Uuid>,
) -> Result<(), ConnectError> {
    // TODO: Delete from _reactor_connect.connection_pairs WHERE id = $1
    // TODO: Clear pair_id from associated connections
    Ok(())
}
