//! Admin routes for job CRUD operations.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};

use crate::audit::{event_types, record_audit_event};
use crate::error::JobsError;
use crate::manifest::BackoffStrategy;
use crate::state::{JobCtx, JobsState};
use crate::store::{Job, JobsStore, NewJob, PgJobsStore, Run, Trigger};
use crate::JOB_NAME_REGEX;

/// Create job request.
#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    /// Job name.
    pub name: String,
    /// Underlying function name in reactor-functions.
    pub function_name: String,
    /// Job description.
    pub description: Option<String>,
    /// Max retry attempts.
    #[serde(default = "default_max_attempts")]
    pub retry_max_attempts: i32,
    /// Backoff strategy.
    #[serde(default)]
    pub retry_backoff: BackoffStrategy,
    /// Initial retry delay in ms.
    #[serde(default = "default_initial_delay")]
    pub retry_initial_delay_ms: i32,
    /// Max retry delay in ms.
    #[serde(default = "default_max_delay")]
    pub retry_max_delay_ms: i32,
    /// Max concurrent runs.
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: i32,
    /// Timeout in ms.
    #[serde(default = "default_timeout")]
    pub timeout_ms: i32,
}

fn default_max_attempts() -> i32 {
    3
}
fn default_initial_delay() -> i32 {
    1000
}
fn default_max_delay() -> i32 {
    60000
}
fn default_max_concurrency() -> i32 {
    10
}
fn default_timeout() -> i32 {
    600000
}

/// Job response.
#[derive(Debug, Serialize)]
pub struct JobResponse {
    /// Job details.
    #[serde(flatten)]
    pub job: Job,
}

/// Job detail response (includes triggers and recent runs).
#[derive(Debug, Serialize)]
pub struct JobDetailResponse {
    /// Job details.
    #[serde(flatten)]
    pub job: Job,
    /// Associated triggers.
    pub triggers: Vec<Trigger>,
    /// Recent runs.
    pub recent_runs: Vec<Run>,
}

/// List jobs response.
#[derive(Debug, Serialize)]
pub struct ListJobsResponse {
    /// List of jobs.
    pub jobs: Vec<Job>,
}

/// Create a new job.
///
/// POST /jobs/v1/_admin/jobs
pub async fn create_job(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Json(req): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<JobResponse>), JobsError> {
    // Check permission
    if !ctx.has_permission("jobs:create") && !ctx.has_permission("*") {
        return Err(JobsError::PermissionDenied("jobs:create".to_string()));
    }

    // Validate job name
    if !JOB_NAME_REGEX.is_match(&req.name) {
        return Err(JobsError::InvalidJobName(req.name));
    }

    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Check if job already exists
    if store.get_job(org_id, &req.name).await?.is_some() {
        return Err(JobsError::InvalidJobName(format!(
            "job '{}' already exists",
            req.name
        )));
    }

    // TODO: Verify function exists in reactor-functions
    // For now, we trust the caller

    // Create the job
    let new_job = NewJob {
        org_id,
        name: req.name.clone(),
        function_name: req.function_name.clone(),
        description: req.description,
        retry_max_attempts: req.retry_max_attempts,
        retry_backoff: req.retry_backoff,
        retry_initial_delay_ms: req.retry_initial_delay_ms,
        retry_max_delay_ms: req.retry_max_delay_ms,
        max_concurrency: req.max_concurrency,
        timeout_ms: req.timeout_ms,
    };

    let job = store.create_job(&new_job).await?;

    // Record audit event
    let _ = record_audit_event(
        &store,
        &ctx,
        event_types::JOB_CREATE,
        Some(job.id),
        None,
        serde_json::json!({
            "name": job.name,
            "function_name": job.function_name,
        }),
    )
    .await;

    Ok((StatusCode::CREATED, Json(JobResponse { job })))
}

/// List all jobs for the organization.
///
/// GET /jobs/v1/_admin/jobs
pub async fn list_jobs(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
) -> Result<Json<ListJobsResponse>, JobsError> {
    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    let jobs = store.list_jobs(org_id).await?;

    Ok(Json(ListJobsResponse { jobs }))
}

/// Get a specific job by name.
///
/// GET /jobs/v1/_admin/jobs/{name}
pub async fn get_job(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(name): Path<String>,
) -> Result<Json<JobDetailResponse>, JobsError> {
    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    let job = store
        .get_job(org_id, &name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(name.clone()))?;

    // Get triggers
    let triggers = store.get_triggers(job.id).await?;

    // Get recent runs
    let recent_runs = store.list_runs(job.id, 10).await?;

    Ok(Json(JobDetailResponse {
        job,
        triggers,
        recent_runs,
    }))
}

/// Delete a job.
///
/// DELETE /jobs/v1/_admin/jobs/{name}
pub async fn delete_job(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(name): Path<String>,
) -> Result<StatusCode, JobsError> {
    // Check permission
    let permission = format!("jobs:{}:admin", name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("*") {
        return Err(JobsError::PermissionDenied(permission));
    }

    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    let job = store
        .get_job(org_id, &name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(name.clone()))?;

    // Delete the job (cascades to triggers, runs via FK)
    store.delete_job(job.id).await?;

    // Record audit event
    let _ = record_audit_event(
        &store,
        &ctx,
        event_types::JOB_DELETE,
        Some(job.id),
        None,
        serde_json::json!({
            "name": job.name,
        }),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}
