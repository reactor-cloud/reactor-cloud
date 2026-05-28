//! Dead Letter Queue (DLQ) routes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::{event_types, record_audit_event};
use crate::error::JobsError;
use crate::state::{JobCtx, JobsState};
use crate::store::{DlqEntry, JobsStore, PgJobsStore, Run};

/// List DLQ response.
#[derive(Debug, Serialize)]
pub struct ListDlqResponse {
    /// DLQ entries.
    pub entries: Vec<DlqEntry>,
}

/// DLQ path parameters.
#[derive(Debug, Deserialize)]
pub struct DlqPath {
    /// Job name.
    pub name: String,
    /// DLQ entry ID.
    pub dlq_id: Uuid,
}

/// Run response.
#[derive(Debug, Serialize)]
pub struct RunResponse {
    /// Run details.
    #[serde(flatten)]
    pub run: Run,
}

/// List DLQ entries for a job.
///
/// GET /jobs/v1/_admin/jobs/{name}/dlq
pub async fn list_dlq(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(name): Path<String>,
) -> Result<Json<ListDlqResponse>, JobsError> {
    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Get the job
    let job = store
        .get_job(org_id, &name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(name.clone()))?;

    let entries = store.list_dlq(job.id, 100).await?;

    Ok(Json(ListDlqResponse { entries }))
}

/// Retry a DLQ entry (creates a new run).
///
/// POST /jobs/v1/_admin/jobs/{name}/dlq/{dlq_id}/retry
pub async fn retry_dlq(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(path): Path<DlqPath>,
) -> Result<(StatusCode, Json<RunResponse>), JobsError> {
    // Check permission
    let permission = format!("jobs:{}:admin", path.name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("*") {
        return Err(JobsError::PermissionDenied(permission));
    }

    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Get the job
    let job = store
        .get_job(org_id, &path.name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(path.name.clone()))?;

    // Retry from DLQ
    let run_id = store.retry_from_dlq(path.dlq_id).await?;

    // Enqueue the new run
    use reactor_cache::QueueOperations;
    let queue_name = format!("jobs:{}", org_id);
    state
        .cache
        .enqueue(&queue_name, run_id.as_bytes(), None)
        .await?;

    // Record audit event
    let _ = record_audit_event(
        &store,
        &ctx,
        event_types::DLQ_RETRY,
        Some(job.id),
        Some(run_id),
        serde_json::json!({
            "dlq_id": path.dlq_id,
        }),
    )
    .await;

    // Get the new run
    let run = store
        .get_run(run_id)
        .await?
        .ok_or_else(|| JobsError::Internal("failed to get created run".to_string()))?;

    Ok((StatusCode::CREATED, Json(RunResponse { run })))
}

/// Delete a DLQ entry (discard without retry).
///
/// DELETE /jobs/v1/_admin/jobs/{name}/dlq/{dlq_id}
pub async fn delete_dlq(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(path): Path<DlqPath>,
) -> Result<StatusCode, JobsError> {
    // Check permission
    let permission = format!("jobs:{}:admin", path.name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("*") {
        return Err(JobsError::PermissionDenied(permission));
    }

    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Get the job
    let job = store
        .get_job(org_id, &path.name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(path.name.clone()))?;

    // Delete the DLQ entry
    store.delete_dlq(path.dlq_id).await?;

    // Record audit event
    let _ = record_audit_event(
        &store,
        &ctx,
        event_types::DLQ_DELETE,
        Some(job.id),
        None,
        serde_json::json!({
            "dlq_id": path.dlq_id,
        }),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}
