//! Audit log routes.

use crate::audit::AuditLogger;
use crate::error::OpsError;
use crate::state::OpsState;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for listing audit entries.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ListAuditQuery {
    /// Maximum number of entries to return.
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Offset for pagination.
    #[serde(default)]
    pub offset: i64,
    /// Filter by actor user ID.
    pub actor_user_id: Option<String>,
}

fn default_limit() -> i64 {
    50
}

/// Audit entry in API response.
#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEntry {
    /// Entry ID.
    pub id: i64,
    /// Timestamp.
    pub timestamp: String,
    /// Actor user ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_user_id: Option<String>,
    /// Actor IP.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_ip: Option<String>,
    /// Action.
    pub action: String,
    /// Scope used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_used: Option<String>,
    /// Resource type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    /// Resource ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    /// Status.
    pub status: String,
    /// Whether step-up was used.
    pub step_up_used: bool,
}

/// Response for listing audit entries.
#[derive(Debug, Serialize, ToSchema)]
pub struct ListAuditResponse {
    /// Audit entries.
    pub entries: Vec<AuditEntry>,
    /// Total count (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}

/// List audit log entries.
#[utoipa::path(
    get,
    path = "/_ops/v1/audit",
    params(ListAuditQuery),
    responses(
        (status = 200, description = "Audit entries", body = ListAuditResponse),
    )
)]
pub async fn list_audit(
    State(state): State<OpsState>,
    Query(query): Query<ListAuditQuery>,
) -> Result<Json<ListAuditResponse>, OpsError> {
    let logger = AuditLogger::new(state.pool.clone());

    let limit = query.limit.min(100).max(1);
    let offset = query.offset.max(0);

    let rows = if let Some(actor_id) = query.actor_user_id {
        let user_id = actor_id.parse::<uuid::Uuid>()
            .map_err(|_| OpsError::Validation("Invalid actor_user_id".to_string()))?;
        logger.query_by_actor(&reactor_core::id::UserId::from(user_id), limit).await?
    } else {
        logger.query_recent(limit, offset).await?
    };

    let entries = rows
        .into_iter()
        .map(|row| AuditEntry {
            id: row.id,
            timestamp: row.ts.to_rfc3339(),
            actor_user_id: row.actor_user_id.map(|u| u.to_string()),
            actor_ip: row.actor_ip,
            action: row.action,
            scope_used: row.scope_used,
            resource_type: row.resource_type,
            resource_id: row.resource_id,
            status: row.status,
            step_up_used: row.step_up_used,
        })
        .collect();

    Ok(Json(ListAuditResponse {
        entries,
        total: None,
    }))
}
