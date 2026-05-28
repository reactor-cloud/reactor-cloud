//! Invoke routes for manual and webhook triggers.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use reactor_cache::QueueOperations;
use serde::{Deserialize, Serialize};

use crate::error::JobsError;
use crate::manifest::TriggerKind;
use crate::routes::triggers::decode_webhook_token;
use crate::state::{JobCtx, JobsState};
use crate::store::{JobsStore, NewRun, PgJobsStore};

/// Manual trigger request.
#[derive(Debug, Deserialize)]
pub struct ManualTriggerRequest {
    /// Optional payload.
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// Trigger response.
#[derive(Debug, Serialize)]
pub struct TriggerResponse {
    /// Run ID.
    pub run_id: String,
    /// Run status.
    pub status: String,
}

/// Manually trigger a job.
///
/// POST /jobs/v1/{name}/trigger
pub async fn manual_trigger(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(name): Path<String>,
    Json(req): Json<ManualTriggerRequest>,
) -> Result<(StatusCode, Json<TriggerResponse>), JobsError> {
    // Check permission
    let permission = format!("jobs:{}:invoke", name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("*") {
        return Err(JobsError::PermissionDenied(permission));
    }

    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Get the job
    let job = store
        .get_job(org_id, &name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(name.clone()))?;

    // Check concurrency limit
    let active_runs = store.count_active_runs(job.id).await?;
    if active_runs >= job.max_concurrency as u32 {
        return Err(JobsError::ConcurrencyExceeded { job: name });
    }

    // Create a new run
    let new_run = NewRun {
        job_id: job.id,
        org_id,
        trigger_id: None,
        trigger_kind: TriggerKind::Manual,
        payload_json: req.payload,
        max_attempts: job.retry_max_attempts,
    };

    let run = store.create_run(&new_run).await?;

    // Enqueue the run
    let queue_name = format!("jobs:{}", org_id);
    state
        .cache
        .enqueue(&queue_name, run.id.as_bytes(), None)
        .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(TriggerResponse {
            run_id: run.id.to_string(),
            status: "pending".to_string(),
        }),
    ))
}

/// Webhook trigger.
///
/// POST /jobs/v1/webhooks/{token}
pub async fn webhook_trigger(
    State(state): State<JobsState>,
    Path(token): Path<String>,
    body: Bytes,
) -> Result<(StatusCode, Json<TriggerResponse>), JobsError> {
    // Decode the webhook token to get the job ID
    let job_id = decode_webhook_token(&state.config.webhook_secret, &token)?;

    let store = PgJobsStore::new(state.pool.clone());

    // Get the job
    let job = store
        .get_job_by_id(job_id)
        .await?
        .ok_or(JobsError::WebhookTokenInvalid)?;

    // Verify the trigger exists and is a webhook trigger
    let trigger = store
        .get_trigger_by_webhook_token(&token)
        .await?
        .ok_or(JobsError::WebhookTokenInvalid)?;

    if !trigger.enabled {
        return Err(JobsError::WebhookTokenInvalid);
    }

    // Check concurrency limit
    let active_runs = store.count_active_runs(job.id).await?;
    if active_runs >= job.max_concurrency as u32 {
        return Err(JobsError::ConcurrencyExceeded { job: job.name });
    }

    // Check payload size
    if body.len() as u64 > state.config.max_payload_bytes {
        return Err(JobsError::PayloadTooLarge {
            size: body.len() as u64,
            max: state.config.max_payload_bytes,
        });
    }

    // Parse body as JSON or wrap as string
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap_or_else(|_| {
        serde_json::json!({
            "raw": String::from_utf8_lossy(&body).to_string()
        })
    });

    // Create a new run
    let new_run = NewRun {
        job_id: job.id,
        org_id: job.org_id,
        trigger_id: Some(trigger.id),
        trigger_kind: TriggerKind::Webhook,
        payload_json: payload,
        max_attempts: job.retry_max_attempts,
    };

    let run = store.create_run(&new_run).await?;

    // Enqueue the run
    let queue_name = format!("jobs:{}", job.org_id);
    state
        .cache
        .enqueue(&queue_name, run.id.as_bytes(), None)
        .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(TriggerResponse {
            run_id: run.id.to_string(),
            status: "pending".to_string(),
        }),
    ))
}
