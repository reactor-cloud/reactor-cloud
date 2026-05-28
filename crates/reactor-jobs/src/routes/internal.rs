//! Internal routes for SDK communication.
//!
//! These routes are called by the TypeScript SDK during job execution
//! to manage steps, state, events, and sleep.

use axum::{
    extract::{Path, State},
    http::StatusCode, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JobsError;
use crate::state::JobsState;
use crate::store::{
    JobsStore, NewEvent, NewStep, PgJobsStore, RunId, StepId, StepStatus,
};

/// Create step request.
#[derive(Debug, Deserialize)]
pub struct CreateStepRequest {
    /// Step name.
    pub name: String,
}

/// Create step response.
#[derive(Debug, Serialize)]
pub struct CreateStepResponse {
    /// Step ID.
    #[serde(rename = "stepId")]
    pub step_id: StepId,
}

/// Update step request.
#[derive(Debug, Deserialize)]
pub struct UpdateStepRequest {
    /// Step status.
    pub status: String,
    /// Step output (for completed steps).
    pub output: Option<serde_json::Value>,
    /// Error message (for failed steps).
    pub error: Option<String>,
}

/// Set state request.
#[derive(Debug, Deserialize)]
pub struct SetStateRequest {
    /// State key.
    pub key: String,
    /// State value.
    pub value: serde_json::Value,
}

/// Emit event request.
#[derive(Debug, Deserialize)]
pub struct EmitEventRequest {
    /// Event topic.
    pub topic: String,
    /// Event payload.
    pub payload: serde_json::Value,
}

/// Sleep request.
#[derive(Debug, Deserialize)]
pub struct SleepRequest {
    /// Sleep step name.
    pub name: String,
    /// Duration in milliseconds.
    #[serde(rename = "durationMs")]
    pub duration_ms: i64,
}

/// Extract run ID from request headers.
fn extract_run_id(headers: &axum::http::HeaderMap) -> Result<RunId, JobsError> {
    headers
        .get("x-reactor-run-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| JobsError::Internal("missing or invalid X-Reactor-Run-Id header".into()))
}

/// Create a step (step is starting).
///
/// POST /_internal/steps
pub async fn create_step(
    State(state): State<JobsState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateStepRequest>,
) -> Result<(StatusCode, Json<CreateStepResponse>), JobsError> {
    let run_id = extract_run_id(&headers)?;
    let store = PgJobsStore::new(state.pool.clone());

    // Check if step already exists (for idempotency)
    if let Some(existing) = store.get_step(run_id, &req.name).await? {
        return Ok((
            StatusCode::OK,
            Json(CreateStepResponse { step_id: existing.id }),
        ));
    }

    // Create the step
    let step = store
        .create_step(&NewStep {
            run_id,
            name: req.name,
            input_json: None,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateStepResponse { step_id: step.id }),
    ))
}

/// Update a step (completed or failed).
///
/// PUT /_internal/steps/{step_id}
pub async fn update_step(
    State(state): State<JobsState>,
    Path(step_id): Path<StepId>,
    Json(req): Json<UpdateStepRequest>,
) -> Result<StatusCode, JobsError> {
    let store = PgJobsStore::new(state.pool.clone());

    let status = match req.status.as_str() {
        "completed" => StepStatus::Completed,
        "failed" => StepStatus::Failed,
        "skipped" => StepStatus::Skipped,
        _ => return Err(JobsError::Internal(format!("invalid step status: {}", req.status))),
    };

    store
        .update_step(step_id, status, req.output.as_ref(), req.error.as_deref())
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Set a state value.
///
/// POST /_internal/state
pub async fn set_state(
    State(state): State<JobsState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<SetStateRequest>,
) -> Result<StatusCode, JobsError> {
    let run_id = extract_run_id(&headers)?;
    let store = PgJobsStore::new(state.pool.clone());

    store.set_state(run_id, &req.key, &req.value).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get a state value.
///
/// GET /_internal/state/{key}
pub async fn get_state(
    State(state): State<JobsState>,
    headers: axum::http::HeaderMap,
    Path(key): Path<String>,
) -> Result<Json<Option<serde_json::Value>>, JobsError> {
    let run_id = extract_run_id(&headers)?;
    let store = PgJobsStore::new(state.pool.clone());

    let value = store.get_state(run_id, &key).await?;

    Ok(Json(value))
}

/// Delete a state value.
///
/// DELETE /_internal/state/{key}
pub async fn delete_state(
    State(state): State<JobsState>,
    headers: axum::http::HeaderMap,
    Path(key): Path<String>,
) -> Result<StatusCode, JobsError> {
    let run_id = extract_run_id(&headers)?;
    let store = PgJobsStore::new(state.pool.clone());

    store.delete_state(run_id, &key).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Emit an event.
///
/// POST /_internal/events
pub async fn emit_event(
    State(state): State<JobsState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<EmitEventRequest>,
) -> Result<StatusCode, JobsError> {
    let run_id = extract_run_id(&headers)?;
    let store = PgJobsStore::new(state.pool.clone());

    // Get the run to find the org_id
    let run = store
        .get_run(run_id)
        .await?
        .ok_or_else(|| JobsError::RunNotFound(run_id.to_string()))?;

    // Create the event
    store
        .emit_event(&NewEvent {
            org_id: run.org_id,
            topic: req.topic,
            payload_json: req.payload,
            emitted_by_run_id: Some(run_id),
        })
        .await?;

    Ok(StatusCode::CREATED)
}

/// Request a sleep (durable wait).
///
/// POST /_internal/sleep
pub async fn request_sleep(
    State(state): State<JobsState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<SleepRequest>,
) -> Result<StatusCode, JobsError> {
    let run_id = extract_run_id(&headers)?;
    let store = PgJobsStore::new(state.pool.clone());

    // Calculate wakeup time
    let wakeup_at = Utc::now() + chrono::Duration::milliseconds(req.duration_ms);

    // Create a step to track the sleep
    let _step = store
        .create_step(&NewStep {
            run_id,
            name: req.name,
            input_json: Some(serde_json::json!({ "durationMs": req.duration_ms })),
        })
        .await?;

    // Set run to sleeping status
    store.set_run_sleeping(run_id, wakeup_at).await?;

    Ok(StatusCode::ACCEPTED)
}
