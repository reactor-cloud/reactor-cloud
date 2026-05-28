//! Trigger CRUD routes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

use crate::audit::{event_types, record_audit_event};
use crate::error::JobsError;
use crate::manifest::TriggerKind;
use crate::state::{JobCtx, JobsState};
use crate::store::{JobsStore, NewTrigger, PgJobsStore, Trigger, TriggerId};
use crate::EVENT_TOPIC_REGEX;

/// Create trigger request.
#[derive(Debug, Deserialize)]
pub struct CreateTriggerRequest {
    /// Trigger kind.
    pub kind: TriggerKind,
    /// Trigger configuration.
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Trigger response.
#[derive(Debug, Serialize)]
pub struct TriggerResponse {
    /// Trigger details.
    #[serde(flatten)]
    pub trigger: Trigger,
}

/// List triggers response.
#[derive(Debug, Serialize)]
pub struct ListTriggersResponse {
    /// List of triggers.
    pub triggers: Vec<Trigger>,
}

/// Create a new trigger for a job.
///
/// POST /jobs/v1/_admin/jobs/{name}/triggers
pub async fn create_trigger(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(name): Path<String>,
    Json(req): Json<CreateTriggerRequest>,
) -> Result<(StatusCode, Json<TriggerResponse>), JobsError> {
    // Check permission
    let permission = format!("jobs:{}:admin", name);
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

    // Validate and process trigger config
    let (config_json, webhook_token, next_trigger_at) = match req.kind {
        TriggerKind::Cron => {
            let schedule = req
                .config
                .get("schedule")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    JobsError::InvalidTriggerConfig("cron trigger requires 'schedule' field".into())
                })?;

            // Validate cron expression
            let parsed = cron::Schedule::from_str(schedule)
                .map_err(|e| JobsError::InvalidCron(format!("{}: {}", schedule, e)))?;

            // Compute next trigger time
            let next = parsed.upcoming(Utc).next();

            (serde_json::json!({ "schedule": schedule }), None, next)
        }
        TriggerKind::Event => {
            let topic = req
                .config
                .get("topic")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    JobsError::InvalidTriggerConfig("event trigger requires 'topic' field".into())
                })?;

            // Validate topic format
            if !EVENT_TOPIC_REGEX.is_match(topic) {
                return Err(JobsError::InvalidEventTopic(topic.to_string()));
            }

            (serde_json::json!({ "topic": topic }), None, None)
        }
        TriggerKind::Webhook => {
            // Generate a unique webhook token
            let token = generate_webhook_token(&state.config.webhook_secret, job.id)?;
            (serde_json::json!({}), Some(token), None)
        }
        TriggerKind::Manual => {
            // Manual triggers don't need config
            (serde_json::json!({}), None, None)
        }
    };

    // Create the trigger
    let new_trigger = NewTrigger {
        job_id: job.id,
        kind: req.kind,
        config_json,
        webhook_token,
        next_trigger_at,
    };

    let trigger = store.create_trigger(&new_trigger).await?;

    // Record audit event
    let _ = record_audit_event(
        &store,
        &ctx,
        event_types::TRIGGER_CREATE,
        Some(job.id),
        None,
        serde_json::json!({
            "trigger_id": trigger.id,
            "kind": req.kind.to_string(),
        }),
    )
    .await;

    Ok((StatusCode::CREATED, Json(TriggerResponse { trigger })))
}

/// List triggers for a job.
///
/// GET /jobs/v1/_admin/jobs/{name}/triggers
pub async fn list_triggers(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(name): Path<String>,
) -> Result<Json<ListTriggersResponse>, JobsError> {
    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Get the job
    let job = store
        .get_job(org_id, &name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(name.clone()))?;

    let triggers = store.get_triggers(job.id).await?;

    Ok(Json(ListTriggersResponse { triggers }))
}

/// Path parameters for trigger operations.
#[derive(Debug, Deserialize)]
pub struct TriggerPath {
    /// Job name.
    pub name: String,
    /// Trigger ID.
    pub trigger_id: TriggerId,
}

/// Delete a trigger.
///
/// DELETE /jobs/v1/_admin/jobs/{name}/triggers/{trigger_id}
pub async fn delete_trigger(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(path): Path<TriggerPath>,
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

    // Verify trigger exists and belongs to this job
    let trigger = store
        .get_trigger(path.trigger_id)
        .await?
        .ok_or_else(|| JobsError::TriggerNotFound(path.trigger_id.to_string()))?;

    if trigger.job_id != job.id {
        return Err(JobsError::TriggerNotFound(path.trigger_id.to_string()));
    }

    // Delete the trigger
    store.delete_trigger(path.trigger_id).await?;

    // Record audit event
    let _ = record_audit_event(
        &store,
        &ctx,
        event_types::TRIGGER_DELETE,
        Some(job.id),
        None,
        serde_json::json!({
            "trigger_id": path.trigger_id,
            "kind": trigger.kind,
        }),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

/// Generate a webhook token for a trigger.
fn generate_webhook_token(secret: &str, job_id: Uuid) -> Result<String, JobsError> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use rand::RngCore;

    // Derive a 256-bit key from the secret
    let key_bytes = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        hasher.finalize()
    };

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| JobsError::Internal(format!("failed to create cipher: {}", e)))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt job_id
    let plaintext = job_id.as_bytes();
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| JobsError::Internal(format!("encryption failed: {}", e)))?;

    // Combine nonce + ciphertext
    let mut token_bytes = Vec::with_capacity(12 + ciphertext.len());
    token_bytes.extend_from_slice(&nonce_bytes);
    token_bytes.extend_from_slice(&ciphertext);

    Ok(URL_SAFE_NO_PAD.encode(&token_bytes))
}

/// Decode a webhook token to get the job ID.
pub fn decode_webhook_token(secret: &str, token: &str) -> Result<Uuid, JobsError> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    let token_bytes = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| JobsError::WebhookTokenInvalid)?;

    if token_bytes.len() < 13 {
        return Err(JobsError::WebhookTokenInvalid);
    }

    // Derive key
    let key_bytes = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        hasher.finalize()
    };

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|_| JobsError::WebhookTokenInvalid)?;

    let nonce = Nonce::from_slice(&token_bytes[..12]);
    let ciphertext = &token_bytes[12..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| JobsError::WebhookTokenInvalid)?;

    Uuid::from_slice(&plaintext).map_err(|_| JobsError::WebhookTokenInvalid)
}
