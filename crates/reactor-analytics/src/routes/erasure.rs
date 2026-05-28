//! GDPR erasure endpoints.

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use crate::audit::{event_types, write_audit};
use crate::error::AnalyticsError;
use crate::state::{AnalyticsCtx, AnalyticsState};
use crate::store::{AnalyticsStore, ErasureLog};

// ---------------------- Request/Response types ----------------------

/// Erase request.
#[derive(Debug, Deserialize)]
pub struct EraseRequest {
    /// Subject kind: "user" or "anonymous".
    pub subject_kind: SubjectKind,
    /// Subject ID (user_id or anonymous_id).
    pub subject_id: String,
}

/// Subject kind enum.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SubjectKind {
    User,
    Anonymous,
}

/// Erase response.
#[derive(Debug, Serialize)]
pub struct EraseResponse {
    pub success: bool,
    pub rows_deleted: u64,
}

/// Export request.
#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    /// Subject kind: "user" only for export.
    pub subject_kind: SubjectKind,
    /// Subject ID (user_id).
    pub subject_id: String,
}

// ---------------------- Handlers ----------------------

/// Erase all data for a user or anonymous ID (GDPR right to erasure).
///
/// POST /analytics/v1/erase
pub async fn erase<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<EraseRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Check permission
    if !ctx.has_permission("analytics:erase") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:erase".to_string(),
        ));
    }

    let outcome = match req.subject_kind {
        SubjectKind::User => {
            state
                .store
                .erase_user(ctx.project_id, &req.subject_id)
                .await?
        }
        SubjectKind::Anonymous => {
            // Also create a tombstone to prevent future events
            state
                .store
                .create_tombstone(ctx.project_id, &req.subject_id, "erased")
                .await?;

            state
                .store
                .erase_anonymous(ctx.project_id, &req.subject_id)
                .await?
        }
    };

    // Log erasure for audit
    let log = ErasureLog {
        project_id: ctx.project_id,
        subject_kind: match req.subject_kind {
            SubjectKind::User => "user".to_string(),
            SubjectKind::Anonymous => "anonymous".to_string(),
        },
        subject_id: req.subject_id.clone(),
        rows_deleted: outcome.rows_deleted,
        actor_user_id: ctx.user_id().map(Into::into),
        request_id: ctx.request_id.to_string(),
    };

    state.store.write_erasure_log(&log).await?;

    // Write audit event
    let event_type = match req.subject_kind {
        SubjectKind::User => event_types::USER_ERASE,
        SubjectKind::Anonymous => event_types::ANON_ERASE,
    };

    write_audit(
        &state.store,
        &ctx,
        event_type,
        Some(ctx.project_id),
        serde_json::json!({
            "subject_kind": log.subject_kind,
            "subject_id": log.subject_id,
            "rows_deleted": log.rows_deleted
        }),
    )
    .await?;

    let response = EraseResponse {
        success: true,
        rows_deleted: outcome.rows_deleted,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Export all data for a user (GDPR right to portability).
///
/// POST /analytics/v1/export
pub async fn export<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<ExportRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Check permission
    if !ctx.has_permission("analytics:export") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:export".to_string(),
        ));
    }

    // Only support user exports
    let events = match req.subject_kind {
        SubjectKind::User => {
            state
                .store
                .export_user(ctx.project_id, &req.subject_id)
                .await?
        }
        SubjectKind::Anonymous => {
            return Err(AnalyticsError::Validation(
                "export only supports user subjects".to_string(),
            ));
        }
    };

    // Return as JSON Lines (one JSON object per line)
    let body = events
        .into_iter()
        .map(|e| {
            let mut line = serde_json::to_string(&e).unwrap_or_default();
            line.push('\n');
            line
        })
        .collect::<String>();

    Ok((
        StatusCode::OK,
        [("content-type", "application/x-ndjson")],
        body,
    ))
}
