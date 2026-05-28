//! Run management routes.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};

use crate::audit::{event_types, record_audit_event};
use crate::error::JobsError;
use crate::state::{JobCtx, JobsState};
use crate::store::{JobsStore, PgJobsStore, Run, RunId, RunStatus, StateEntry, Step};

/// Query parameters for listing runs.
#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    /// Filter by status.
    pub status: Option<String>,
    /// Limit number of results.
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    50
}

/// List runs response.
#[derive(Debug, Serialize)]
pub struct ListRunsResponse {
    /// List of runs.
    pub runs: Vec<Run>,
}

/// Run detail response.
#[derive(Debug, Serialize)]
pub struct RunDetailResponse {
    /// Run details.
    #[serde(flatten)]
    pub run: Run,
    /// Steps in this run.
    pub steps: Vec<Step>,
    /// State entries.
    pub state: Vec<StateEntry>,
}

/// Run response (minimal).
#[derive(Debug, Serialize)]
pub struct RunResponse {
    /// Run details.
    #[serde(flatten)]
    pub run: Run,
}

/// Path parameters for run operations.
#[derive(Debug, Deserialize)]
pub struct RunPath {
    /// Job name.
    pub name: String,
    /// Run ID.
    pub run_id: RunId,
}

/// List runs for a job.
///
/// GET /jobs/v1/_admin/jobs/{name}/runs
pub async fn list_runs(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(name): Path<String>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<ListRunsResponse>, JobsError> {
    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Get the job
    let job = store
        .get_job(org_id, &name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(name.clone()))?;

    let runs = store.list_runs(job.id, query.limit).await?;

    // Filter by status if specified
    let runs = if let Some(status) = query.status {
        runs.into_iter().filter(|r| r.status == status).collect()
    } else {
        runs
    };

    Ok(Json(ListRunsResponse { runs }))
}

/// Get a specific run.
///
/// GET /jobs/v1/_admin/jobs/{name}/runs/{run_id}
pub async fn get_run(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(path): Path<RunPath>,
) -> Result<Json<RunDetailResponse>, JobsError> {
    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Get the job
    let job = store
        .get_job(org_id, &path.name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(path.name.clone()))?;

    // Get the run
    let run = store
        .get_run(path.run_id)
        .await?
        .ok_or_else(|| JobsError::RunNotFound(path.run_id.to_string()))?;

    // Verify run belongs to this job
    if run.job_id != job.id {
        return Err(JobsError::RunNotFound(path.run_id.to_string()));
    }

    // Get steps and state
    let steps = store.list_steps(path.run_id).await?;
    let state_entries = store.list_state(path.run_id).await?;

    Ok(Json(RunDetailResponse {
        run,
        steps,
        state: state_entries,
    }))
}

/// Cancel a run.
///
/// POST /jobs/v1/_admin/jobs/{name}/runs/{run_id}/cancel
pub async fn cancel_run(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(path): Path<RunPath>,
) -> Result<Json<RunResponse>, JobsError> {
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

    // Get the run
    let run = store
        .get_run(path.run_id)
        .await?
        .ok_or_else(|| JobsError::RunNotFound(path.run_id.to_string()))?;

    // Verify run belongs to this job
    if run.job_id != job.id {
        return Err(JobsError::RunNotFound(path.run_id.to_string()));
    }

    // Check if run can be cancelled
    if run.status == "succeeded" || run.status == "failed" || run.status == "cancelled" {
        return Err(JobsError::RunAlreadyComplete(path.run_id.to_string()));
    }

    // Cancel the run
    store
        .update_run_status(path.run_id, RunStatus::Cancelled, None, None)
        .await?;

    // Record audit event
    let _ = record_audit_event(
        &store,
        &ctx,
        event_types::RUN_CANCEL,
        Some(job.id),
        Some(path.run_id),
        serde_json::json!({}),
    )
    .await;

    // Get updated run
    let run = store
        .get_run(path.run_id)
        .await?
        .ok_or_else(|| JobsError::RunNotFound(path.run_id.to_string()))?;

    Ok(Json(RunResponse { run }))
}

/// Retry a run (creates a new run with same payload).
///
/// POST /jobs/v1/_admin/jobs/{name}/runs/{run_id}/retry
pub async fn retry_run(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(path): Path<RunPath>,
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

    // Get the original run
    let original_run = store
        .get_run(path.run_id)
        .await?
        .ok_or_else(|| JobsError::RunNotFound(path.run_id.to_string()))?;

    // Verify run belongs to this job
    if original_run.job_id != job.id {
        return Err(JobsError::RunNotFound(path.run_id.to_string()));
    }

    // Create a new run with the same payload
    use crate::manifest::TriggerKind;
    use crate::store::NewRun;

    let new_run = NewRun {
        job_id: job.id,
        org_id,
        trigger_id: original_run.trigger_id,
        trigger_kind: TriggerKind::Manual, // Retry counts as manual
        payload_json: original_run.payload_json,
        max_attempts: job.retry_max_attempts,
    };

    let run = store.create_run(&new_run).await?;

    // Enqueue the run
    use reactor_cache::QueueOperations;
    let queue_name = format!("jobs:{}", org_id);
    state
        .cache
        .enqueue(&queue_name, run.id.as_bytes(), None)
        .await?;

    // Record audit event
    let _ = record_audit_event(
        &store,
        &ctx,
        event_types::RUN_RETRY,
        Some(job.id),
        Some(run.id),
        serde_json::json!({
            "original_run_id": path.run_id,
        }),
    )
    .await;

    Ok((StatusCode::CREATED, Json(RunResponse { run })))
}
