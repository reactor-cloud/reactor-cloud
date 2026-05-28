//! Audit event recording.

use uuid::Uuid;

use crate::error::JobsError;
use crate::state::JobCtx;
use crate::store::{AuditEvent, JobId, JobsStore, PgJobsStore, RunId};

/// Audit event types.
pub mod event_types {
    /// Job created event.
    pub const JOB_CREATE: &str = "job.create";
    /// Job deleted event.
    pub const JOB_DELETE: &str = "job.delete";
    /// Trigger created event.
    pub const TRIGGER_CREATE: &str = "trigger.create";
    /// Trigger deleted event.
    pub const TRIGGER_DELETE: &str = "trigger.delete";
    /// Trigger disabled event.
    pub const TRIGGER_DISABLE: &str = "trigger.disable";
    /// Trigger enabled event.
    pub const TRIGGER_ENABLE: &str = "trigger.enable";
    /// Run cancelled event.
    pub const RUN_CANCEL: &str = "run.cancel";
    /// Run retried event.
    pub const RUN_RETRY: &str = "run.retry";
    /// DLQ entry retried event.
    pub const DLQ_RETRY: &str = "dlq.retry";
    /// DLQ entry deleted event.
    pub const DLQ_DELETE: &str = "dlq.delete";
}

/// Record an audit event.
pub async fn record_audit_event(
    store: &PgJobsStore,
    ctx: &JobCtx,
    event_type: &str,
    job_id: Option<JobId>,
    run_id: Option<RunId>,
    details: serde_json::Value,
) -> Result<(), JobsError> {
    let user_id: Option<Uuid> = ctx.user_id().map(|id| id.into_uuid());
    let org_id: Uuid = (*ctx.active_org()).into_uuid();
    
    let event = AuditEvent {
        event_type: event_type.to_string(),
        actor_user_id: user_id,
        actor_apikey_id: None,
        org_id: Some(org_id),
        job_id,
        run_id,
        details,
        request_id: ctx.request_id.clone(),
    };

    store.write_audit_event(&event).await
}
