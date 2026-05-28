//! Conflict policy management endpoints.
//!
//! Defines how conflicts are resolved during bidirectional sync.

use crate::error::ConnectError;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::ConnectStore;
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Conflict policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictPolicy {
    /// Policy ID.
    pub id: Uuid,
    /// Policy name.
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// Policy type.
    pub policy_type: ConflictPolicyType,
    /// Custom rules (for custom policies).
    #[serde(default)]
    pub rules: Vec<ConflictRule>,
    /// Whether this is the default policy.
    pub is_default: bool,
    /// Whether this policy is enabled.
    pub enabled: bool,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Updated at.
    pub updated_at: DateTime<Utc>,
}

/// Named conflict policy types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicyType {
    /// Source always wins.
    SourceWins,
    /// Destination always wins.
    DestWins,
    /// Most recently modified wins.
    LatestWins,
    /// Custom rules.
    Custom,
}

/// A conflict resolution rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRule {
    /// Stream pattern (glob, e.g., "Lead", "*").
    pub stream: String,
    /// Field pattern (glob, e.g., "Email", "*").
    #[serde(default)]
    pub field: Option<String>,
    /// Condition (optional).
    #[serde(default)]
    pub when: Option<ConflictCondition>,
    /// Resolution action.
    pub then: ConflictResolution,
}

/// Conflict condition (when clause).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConflictCondition {
    /// Source A field equals value.
    SourceAEquals { field: String, value: serde_json::Value },
    /// Source B field equals value.
    SourceBEquals { field: String, value: serde_json::Value },
    /// Field is in list.
    FieldIn { values: Vec<String> },
    /// Always true.
    Always,
}

/// Conflict resolution action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Prefer source A.
    PreferSourceA,
    /// Prefer source B.
    PreferSourceB,
    /// Prefer the most recently modified.
    PreferLatest,
    /// Merge (take non-null values from both).
    Merge,
    /// Skip (don't update).
    Skip,
}

/// Create policy request.
#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    /// Policy name.
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// Policy type.
    pub policy_type: ConflictPolicyType,
    /// Custom rules (for custom policies).
    #[serde(default)]
    pub rules: Vec<ConflictRule>,
    /// Whether this is the default policy.
    #[serde(default)]
    pub is_default: bool,
}

/// GET /connect/v1/policies
pub async fn list<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
) -> Result<Json<Vec<ConflictPolicy>>, ConnectError> {
    // TODO: Query _reactor_connect.conflict_policies
    Ok(Json(vec![]))
}

/// POST /connect/v1/policies
pub async fn create<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Json(req): Json<CreatePolicyRequest>,
) -> Result<Json<ConflictPolicy>, ConnectError> {
    let now = Utc::now();
    let policy = ConflictPolicy {
        id: Uuid::new_v4(),
        name: req.name,
        description: req.description,
        policy_type: req.policy_type,
        rules: req.rules,
        is_default: req.is_default,
        enabled: true,
        created_at: now,
        updated_at: now,
    };

    // TODO: Insert into _reactor_connect.conflict_policies

    Ok(Json(policy))
}

/// GET /connect/v1/policies/:id
pub async fn get<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConflictPolicy>, ConnectError> {
    // TODO: Query _reactor_connect.conflict_policies WHERE id = $1
    Err(ConnectError::InvalidInput(format!("Policy {} not found", id)))
}

/// DELETE /connect/v1/policies/:id
pub async fn delete<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(id): Path<Uuid>,
) -> Result<(), ConnectError> {
    // TODO: Delete from _reactor_connect.conflict_policies WHERE id = $1
    Ok(())
}
