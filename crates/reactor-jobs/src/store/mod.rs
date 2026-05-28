//! Job store traits and implementations.

mod postgres;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JobsError;
use crate::manifest::{BackoffStrategy, TriggerKind};

pub use postgres::PgJobsStore;

/// Job ID type.
pub type JobId = Uuid;
/// Trigger ID type.
pub type TriggerId = Uuid;
/// Run ID type.
pub type RunId = Uuid;
/// Step ID type.
pub type StepId = Uuid;
/// Event ID type.
pub type EventId = Uuid;

/// Run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    /// Waiting to be picked up.
    Pending,
    /// Waiting for concurrency slot.
    Queued,
    /// Currently executing.
    Running,
    /// Waiting for sleep to expire.
    Sleeping,
    /// Completed successfully.
    Succeeded,
    /// Failed after all retries.
    Failed,
    /// Manually cancelled.
    Cancelled,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::Sleeping => write!(f, "sleeping"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Step status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    /// Not started.
    Pending,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Skipped (e.g., due to condition).
    Skipped,
}

/// A job definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Job ID.
    pub id: JobId,
    /// Organization ID.
    pub org_id: Uuid,
    /// Job name.
    pub name: String,
    /// Underlying function name.
    pub function_name: String,
    /// Description.
    pub description: Option<String>,
    /// Max retry attempts.
    pub retry_max_attempts: i32,
    /// Backoff strategy.
    pub retry_backoff: String,
    /// Initial backoff delay in ms.
    pub retry_initial_delay_ms: i32,
    /// Max backoff delay in ms.
    pub retry_max_delay_ms: i32,
    /// Max concurrent runs.
    pub max_concurrency: i32,
    /// Timeout in ms.
    pub timeout_ms: i32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// New job creation.
#[derive(Debug, Clone)]
pub struct NewJob {
    /// Organization ID.
    pub org_id: Uuid,
    /// Job name.
    pub name: String,
    /// Underlying function name.
    pub function_name: String,
    /// Description.
    pub description: Option<String>,
    /// Max retry attempts.
    pub retry_max_attempts: i32,
    /// Backoff strategy.
    pub retry_backoff: BackoffStrategy,
    /// Initial backoff delay in ms.
    pub retry_initial_delay_ms: i32,
    /// Max backoff delay in ms.
    pub retry_max_delay_ms: i32,
    /// Max concurrent runs.
    pub max_concurrency: i32,
    /// Timeout in ms.
    pub timeout_ms: i32,
}

/// A trigger definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    /// Trigger ID.
    pub id: TriggerId,
    /// Job ID.
    pub job_id: JobId,
    /// Trigger kind.
    pub kind: String,
    /// Configuration JSON.
    pub config_json: serde_json::Value,
    /// Webhook token (for webhook triggers).
    pub webhook_token: Option<String>,
    /// Whether the trigger is enabled.
    pub enabled: bool,
    /// Last triggered timestamp.
    pub last_triggered_at: Option<DateTime<Utc>>,
    /// Next trigger timestamp (for cron).
    pub next_trigger_at: Option<DateTime<Utc>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// New trigger creation.
#[derive(Debug, Clone)]
pub struct NewTrigger {
    /// Job ID.
    pub job_id: JobId,
    /// Trigger kind.
    pub kind: TriggerKind,
    /// Configuration JSON.
    pub config_json: serde_json::Value,
    /// Webhook token (for webhook triggers).
    pub webhook_token: Option<String>,
    /// Next trigger timestamp (for cron).
    pub next_trigger_at: Option<DateTime<Utc>>,
}

/// A job run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    /// Run ID.
    pub id: RunId,
    /// Job ID.
    pub job_id: JobId,
    /// Organization ID.
    pub org_id: Uuid,
    /// Trigger ID.
    pub trigger_id: Option<TriggerId>,
    /// Trigger kind.
    pub trigger_kind: String,
    /// Run status.
    pub status: String,
    /// Payload JSON.
    pub payload_json: serde_json::Value,
    /// Current attempt number.
    pub attempt: i32,
    /// Max attempts.
    pub max_attempts: i32,
    /// Start timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Finish timestamp.
    pub finished_at: Option<DateTime<Utc>>,
    /// Wakeup timestamp (for sleeping runs).
    pub wakeup_at: Option<DateTime<Utc>>,
    /// Error code.
    pub error_code: Option<String>,
    /// Error message.
    pub error_message: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// New run creation.
#[derive(Debug, Clone)]
pub struct NewRun {
    /// Job ID.
    pub job_id: JobId,
    /// Organization ID.
    pub org_id: Uuid,
    /// Trigger ID.
    pub trigger_id: Option<TriggerId>,
    /// Trigger kind.
    pub trigger_kind: TriggerKind,
    /// Payload JSON.
    pub payload_json: serde_json::Value,
    /// Max attempts.
    pub max_attempts: i32,
}

/// A step within a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step ID.
    pub id: StepId,
    /// Run ID.
    pub run_id: RunId,
    /// Step name.
    pub name: String,
    /// Step status.
    pub status: String,
    /// Input JSON.
    pub input_json: Option<serde_json::Value>,
    /// Output JSON (cached result).
    pub output_json: Option<serde_json::Value>,
    /// Attempt number.
    pub attempt: i32,
    /// Start timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Finish timestamp.
    pub finished_at: Option<DateTime<Utc>>,
    /// Error message.
    pub error_message: Option<String>,
}

/// New step creation.
#[derive(Debug, Clone)]
pub struct NewStep {
    /// Run ID.
    pub run_id: RunId,
    /// Step name.
    pub name: String,
    /// Input JSON.
    pub input_json: Option<serde_json::Value>,
}

/// State entry (per-run KV).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    /// Key.
    pub key: String,
    /// Value JSON.
    pub value_json: serde_json::Value,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// An internal event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event ID.
    pub id: EventId,
    /// Organization ID.
    pub org_id: Uuid,
    /// Event topic.
    pub topic: String,
    /// Payload JSON.
    pub payload_json: serde_json::Value,
    /// Run that emitted this event.
    pub emitted_by_run_id: Option<RunId>,
    /// Run that consumed this event.
    pub consumed_by_run_id: Option<RunId>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Consumption timestamp.
    pub consumed_at: Option<DateTime<Utc>>,
}

/// New event creation.
#[derive(Debug, Clone)]
pub struct NewEvent {
    /// Organization ID.
    pub org_id: Uuid,
    /// Event topic.
    pub topic: String,
    /// Payload JSON.
    pub payload_json: serde_json::Value,
    /// Run that emitted this event.
    pub emitted_by_run_id: Option<RunId>,
}

/// DLQ entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqEntry {
    /// DLQ entry ID.
    pub id: Uuid,
    /// Original run ID.
    pub run_id: RunId,
    /// Job ID.
    pub job_id: JobId,
    /// Organization ID.
    pub org_id: Uuid,
    /// Payload JSON.
    pub payload_json: serde_json::Value,
    /// Error code.
    pub error_code: Option<String>,
    /// Error message.
    pub error_message: Option<String>,
    /// Final attempt number.
    pub attempt: i32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Audit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Event type.
    pub event_type: String,
    /// Actor user ID.
    pub actor_user_id: Option<Uuid>,
    /// Actor API key ID.
    pub actor_apikey_id: Option<Uuid>,
    /// Organization ID.
    pub org_id: Option<Uuid>,
    /// Job ID.
    pub job_id: Option<JobId>,
    /// Run ID.
    pub run_id: Option<RunId>,
    /// Details JSON.
    pub details: serde_json::Value,
    /// Request ID.
    pub request_id: String,
}

/// Jobs store trait.
#[async_trait]
pub trait JobsStore: Send + Sync + 'static {
    /// Create a job.
    async fn create_job(&self, job: &NewJob) -> Result<Job, JobsError>;
    /// Get a job by org and name.
    async fn get_job(&self, org_id: Uuid, name: &str) -> Result<Option<Job>, JobsError>;
    /// Get a job by ID.
    async fn get_job_by_id(&self, id: JobId) -> Result<Option<Job>, JobsError>;
    /// List jobs for an org.
    async fn list_jobs(&self, org_id: Uuid) -> Result<Vec<Job>, JobsError>;
    /// Delete a job.
    async fn delete_job(&self, id: JobId) -> Result<(), JobsError>;

    /// Create a trigger.
    async fn create_trigger(&self, trigger: &NewTrigger) -> Result<Trigger, JobsError>;
    /// Get triggers for a job.
    async fn get_triggers(&self, job_id: JobId) -> Result<Vec<Trigger>, JobsError>;
    /// Get a trigger by ID.
    async fn get_trigger(&self, id: TriggerId) -> Result<Option<Trigger>, JobsError>;
    /// Get trigger by webhook token.
    async fn get_trigger_by_webhook_token(&self, token: &str) -> Result<Option<Trigger>, JobsError>;
    /// Delete a trigger.
    async fn delete_trigger(&self, id: TriggerId) -> Result<(), JobsError>;
    /// List due cron triggers.
    async fn list_due_cron_triggers(&self, now: DateTime<Utc>) -> Result<Vec<Trigger>, JobsError>;
    /// Update trigger after firing.
    async fn update_trigger_fired(
        &self,
        id: TriggerId,
        last_triggered_at: DateTime<Utc>,
        next_trigger_at: Option<DateTime<Utc>>,
    ) -> Result<(), JobsError>;

    /// Create a run.
    async fn create_run(&self, run: &NewRun) -> Result<Run, JobsError>;
    /// Get a run by ID.
    async fn get_run(&self, id: RunId) -> Result<Option<Run>, JobsError>;
    /// Update run status.
    async fn update_run_status(
        &self,
        id: RunId,
        status: RunStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), JobsError>;
    /// Set run to sleeping.
    async fn set_run_sleeping(&self, id: RunId, wakeup_at: DateTime<Utc>) -> Result<(), JobsError>;
    /// Increment run attempt.
    async fn increment_run_attempt(&self, id: RunId) -> Result<(), JobsError>;
    /// List runs for a job.
    async fn list_runs(&self, job_id: JobId, limit: u32) -> Result<Vec<Run>, JobsError>;
    /// List sleeping runs that are due.
    async fn list_sleeping_runs_due(&self, now: DateTime<Utc>) -> Result<Vec<Run>, JobsError>;
    /// Count active runs for a job.
    async fn count_active_runs(&self, job_id: JobId) -> Result<u32, JobsError>;
    /// Count active runs for an org.
    async fn count_active_runs_for_org(&self, org_id: Uuid) -> Result<u32, JobsError>;

    /// Create a step.
    async fn create_step(&self, step: &NewStep) -> Result<Step, JobsError>;
    /// Get a step by run and name.
    async fn get_step(&self, run_id: RunId, name: &str) -> Result<Option<Step>, JobsError>;
    /// Update step status and output.
    async fn update_step(
        &self,
        id: StepId,
        status: StepStatus,
        output: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<(), JobsError>;
    /// List steps for a run.
    async fn list_steps(&self, run_id: RunId) -> Result<Vec<Step>, JobsError>;

    /// Get state value.
    async fn get_state(
        &self,
        run_id: RunId,
        key: &str,
    ) -> Result<Option<serde_json::Value>, JobsError>;
    /// Set state value.
    async fn set_state(
        &self,
        run_id: RunId,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), JobsError>;
    /// Delete state value.
    async fn delete_state(&self, run_id: RunId, key: &str) -> Result<(), JobsError>;
    /// List all state for a run.
    async fn list_state(&self, run_id: RunId) -> Result<Vec<StateEntry>, JobsError>;

    /// Emit an event.
    async fn emit_event(&self, event: &NewEvent) -> Result<Event, JobsError>;
    /// Consume an event.
    async fn consume_event(&self, id: EventId, run_id: RunId) -> Result<(), JobsError>;
    /// List pending events for a topic.
    async fn list_pending_events(&self, topic: &str, limit: u32) -> Result<Vec<Event>, JobsError>;
    /// List pending events for an org.
    async fn list_pending_events_for_org(
        &self,
        org_id: Uuid,
        limit: u32,
    ) -> Result<Vec<Event>, JobsError>;

    /// Move a run to DLQ.
    async fn move_to_dlq(&self, run_id: RunId, reason: &str) -> Result<(), JobsError>;
    /// List DLQ entries for a job.
    async fn list_dlq(&self, job_id: JobId, limit: u32) -> Result<Vec<DlqEntry>, JobsError>;
    /// Retry from DLQ.
    async fn retry_from_dlq(&self, dlq_id: Uuid) -> Result<RunId, JobsError>;
    /// Delete DLQ entry.
    async fn delete_dlq(&self, dlq_id: Uuid) -> Result<(), JobsError>;

    /// Write an audit event.
    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), JobsError>;
}

/// Jobs transaction trait for transactional operations.
#[async_trait]
pub trait JobsTx: Send + Sync {
    /// Commit the transaction.
    async fn commit(self) -> Result<(), JobsError>;
    /// Rollback the transaction.
    async fn rollback(self) -> Result<(), JobsError>;
}
