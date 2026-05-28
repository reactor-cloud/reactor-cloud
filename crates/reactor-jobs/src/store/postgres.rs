//! PostgreSQL-backed jobs store.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::JobsError;
use crate::manifest::BackoffStrategy;
use crate::store::{
    AuditEvent, DlqEntry, Event, EventId, Job, JobId, JobsStore, NewEvent, NewJob, NewRun, NewStep,
    NewTrigger, Run, RunId, RunStatus, StateEntry, Step, StepId, StepStatus, Trigger, TriggerId,
};

/// PostgreSQL-backed jobs store.
#[derive(Debug, Clone)]
pub struct PgJobsStore {
    pool: PgPool,
}

impl PgJobsStore {
    /// Create a new PostgreSQL jobs store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run migrations.
    pub async fn migrate(&self) -> Result<(), JobsError> {
        sqlx::raw_sql(MIGRATION_SQL)
            .execute(&self.pool)
            .await
            .map_err(JobsError::Database)?;
        Ok(())
    }
}

#[async_trait]
impl JobsStore for PgJobsStore {
    async fn create_job(&self, job: &NewJob) -> Result<Job, JobsError> {
        let id = Uuid::now_v7();
        let backoff = match job.retry_backoff {
            BackoffStrategy::Linear => "linear",
            BackoffStrategy::Exponential => "exponential",
        };

        let row = sqlx::query_as::<_, JobRow>(
            r#"
            INSERT INTO _reactor_jobs.jobs (
                id, org_id, name, function_name, description,
                retry_max_attempts, retry_backoff, retry_initial_delay_ms, retry_max_delay_ms,
                max_concurrency, timeout_ms
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(job.org_id)
        .bind(&job.name)
        .bind(&job.function_name)
        .bind(&job.description)
        .bind(job.retry_max_attempts)
        .bind(backoff)
        .bind(job.retry_initial_delay_ms)
        .bind(job.retry_max_delay_ms)
        .bind(job.max_concurrency)
        .bind(job.timeout_ms)
        .fetch_one(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.into())
    }

    async fn get_job(&self, org_id: Uuid, name: &str) -> Result<Option<Job>, JobsError> {
        let row = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT * FROM _reactor_jobs.jobs
            WHERE org_id = $1 AND name = $2
            "#,
        )
        .bind(org_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.map(Into::into))
    }

    async fn get_job_by_id(&self, id: JobId) -> Result<Option<Job>, JobsError> {
        let row = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT * FROM _reactor_jobs.jobs WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.map(Into::into))
    }

    async fn list_jobs(&self, org_id: Uuid) -> Result<Vec<Job>, JobsError> {
        let rows = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT * FROM _reactor_jobs.jobs
            WHERE org_id = $1
            ORDER BY name
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn delete_job(&self, id: JobId) -> Result<(), JobsError> {
        sqlx::query("DELETE FROM _reactor_jobs.jobs WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn create_trigger(&self, trigger: &NewTrigger) -> Result<Trigger, JobsError> {
        let id = Uuid::now_v7();
        let kind = trigger.kind.to_string();

        let row = sqlx::query_as::<_, TriggerRow>(
            r#"
            INSERT INTO _reactor_jobs.triggers (
                id, job_id, kind, config_json, webhook_token, next_trigger_at
            ) VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(trigger.job_id)
        .bind(&kind)
        .bind(&trigger.config_json)
        .bind(&trigger.webhook_token)
        .bind(trigger.next_trigger_at)
        .fetch_one(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.into())
    }

    async fn get_triggers(&self, job_id: JobId) -> Result<Vec<Trigger>, JobsError> {
        let rows = sqlx::query_as::<_, TriggerRow>(
            r#"
            SELECT * FROM _reactor_jobs.triggers
            WHERE job_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_trigger(&self, id: TriggerId) -> Result<Option<Trigger>, JobsError> {
        let row = sqlx::query_as::<_, TriggerRow>(
            r#"
            SELECT * FROM _reactor_jobs.triggers WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.map(Into::into))
    }

    async fn get_trigger_by_webhook_token(&self, token: &str) -> Result<Option<Trigger>, JobsError> {
        let row = sqlx::query_as::<_, TriggerRow>(
            r#"
            SELECT * FROM _reactor_jobs.triggers WHERE webhook_token = $1
            "#,
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.map(Into::into))
    }

    async fn delete_trigger(&self, id: TriggerId) -> Result<(), JobsError> {
        sqlx::query("DELETE FROM _reactor_jobs.triggers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn list_due_cron_triggers(&self, now: DateTime<Utc>) -> Result<Vec<Trigger>, JobsError> {
        let rows = sqlx::query_as::<_, TriggerRow>(
            r#"
            SELECT * FROM _reactor_jobs.triggers
            WHERE kind = 'cron' AND enabled = true AND next_trigger_at <= $1
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update_trigger_fired(
        &self,
        id: TriggerId,
        last_triggered_at: DateTime<Utc>,
        next_trigger_at: Option<DateTime<Utc>>,
    ) -> Result<(), JobsError> {
        sqlx::query(
            r#"
            UPDATE _reactor_jobs.triggers
            SET last_triggered_at = $2, next_trigger_at = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(last_triggered_at)
        .bind(next_trigger_at)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn create_run(&self, run: &NewRun) -> Result<Run, JobsError> {
        let id = Uuid::now_v7();
        let trigger_kind = run.trigger_kind.to_string();

        let row = sqlx::query_as::<_, RunRow>(
            r#"
            INSERT INTO _reactor_jobs.runs (
                id, job_id, org_id, trigger_id, trigger_kind,
                payload_json, max_attempts
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(run.job_id)
        .bind(run.org_id)
        .bind(run.trigger_id)
        .bind(&trigger_kind)
        .bind(&run.payload_json)
        .bind(run.max_attempts)
        .fetch_one(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.into())
    }

    async fn get_run(&self, id: RunId) -> Result<Option<Run>, JobsError> {
        let row = sqlx::query_as::<_, RunRow>(
            r#"
            SELECT * FROM _reactor_jobs.runs WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.map(Into::into))
    }

    async fn update_run_status(
        &self,
        id: RunId,
        status: RunStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), JobsError> {
        let status_str = status.to_string();
        let finished_at = match status {
            RunStatus::Succeeded | RunStatus::Failed | RunStatus::Cancelled => Some(Utc::now()),
            _ => None,
        };
        let started_at = match status {
            RunStatus::Running => Some(Utc::now()),
            _ => None,
        };

        sqlx::query(
            r#"
            UPDATE _reactor_jobs.runs
            SET status = $2,
                started_at = COALESCE($3, started_at),
                finished_at = COALESCE($4, finished_at),
                error_code = COALESCE($5, error_code),
                error_message = COALESCE($6, error_message)
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(&status_str)
        .bind(started_at)
        .bind(finished_at)
        .bind(error_code)
        .bind(error_message)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn set_run_sleeping(&self, id: RunId, wakeup_at: DateTime<Utc>) -> Result<(), JobsError> {
        sqlx::query(
            r#"
            UPDATE _reactor_jobs.runs
            SET status = 'sleeping', wakeup_at = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(wakeup_at)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn increment_run_attempt(&self, id: RunId) -> Result<(), JobsError> {
        sqlx::query(
            r#"
            UPDATE _reactor_jobs.runs
            SET attempt = attempt + 1, status = 'pending'
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn list_runs(&self, job_id: JobId, limit: u32) -> Result<Vec<Run>, JobsError> {
        let rows = sqlx::query_as::<_, RunRow>(
            r#"
            SELECT * FROM _reactor_jobs.runs
            WHERE job_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(job_id)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_sleeping_runs_due(&self, now: DateTime<Utc>) -> Result<Vec<Run>, JobsError> {
        let rows = sqlx::query_as::<_, RunRow>(
            r#"
            SELECT * FROM _reactor_jobs.runs
            WHERE status = 'sleeping' AND wakeup_at <= $1
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn count_active_runs(&self, job_id: JobId) -> Result<u32, JobsError> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM _reactor_jobs.runs
            WHERE job_id = $1 AND status IN ('pending', 'running', 'sleeping', 'queued')
            "#,
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.0 as u32)
    }

    async fn count_active_runs_for_org(&self, org_id: Uuid) -> Result<u32, JobsError> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM _reactor_jobs.runs
            WHERE org_id = $1 AND status IN ('pending', 'running', 'sleeping', 'queued')
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.0 as u32)
    }

    async fn create_step(&self, step: &NewStep) -> Result<Step, JobsError> {
        let id = Uuid::now_v7();

        let row = sqlx::query_as::<_, StepRow>(
            r#"
            INSERT INTO _reactor_jobs.steps (id, run_id, name, input_json, started_at, status)
            VALUES ($1, $2, $3, $4, now(), 'running')
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(step.run_id)
        .bind(&step.name)
        .bind(&step.input_json)
        .fetch_one(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.into())
    }

    async fn get_step(&self, run_id: RunId, name: &str) -> Result<Option<Step>, JobsError> {
        let row = sqlx::query_as::<_, StepRow>(
            r#"
            SELECT * FROM _reactor_jobs.steps
            WHERE run_id = $1 AND name = $2
            "#,
        )
        .bind(run_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.map(Into::into))
    }

    async fn update_step(
        &self,
        id: StepId,
        status: StepStatus,
        output: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<(), JobsError> {
        let status_str = match status {
            StepStatus::Pending => "pending",
            StepStatus::Running => "running",
            StepStatus::Completed => "completed",
            StepStatus::Failed => "failed",
            StepStatus::Skipped => "skipped",
        };

        sqlx::query(
            r#"
            UPDATE _reactor_jobs.steps
            SET status = $2, output_json = COALESCE($3, output_json),
                error_message = $4, finished_at = now()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status_str)
        .bind(output)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn list_steps(&self, run_id: RunId) -> Result<Vec<Step>, JobsError> {
        let rows = sqlx::query_as::<_, StepRow>(
            r#"
            SELECT * FROM _reactor_jobs.steps
            WHERE run_id = $1
            ORDER BY started_at
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_state(
        &self,
        run_id: RunId,
        key: &str,
    ) -> Result<Option<serde_json::Value>, JobsError> {
        let row: Option<(serde_json::Value,)> = sqlx::query_as(
            r#"
            SELECT value_json FROM _reactor_jobs.state
            WHERE run_id = $1 AND key = $2
            "#,
        )
        .bind(run_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.map(|r| r.0))
    }

    async fn set_state(
        &self,
        run_id: RunId,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), JobsError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_jobs.state (run_id, key, value_json)
            VALUES ($1, $2, $3)
            ON CONFLICT (run_id, key) DO UPDATE
            SET value_json = EXCLUDED.value_json, updated_at = now()
            "#,
        )
        .bind(run_id)
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn delete_state(&self, run_id: RunId, key: &str) -> Result<(), JobsError> {
        sqlx::query(
            r#"
            DELETE FROM _reactor_jobs.state
            WHERE run_id = $1 AND key = $2
            "#,
        )
        .bind(run_id)
        .bind(key)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn list_state(&self, run_id: RunId) -> Result<Vec<StateEntry>, JobsError> {
        let rows = sqlx::query_as::<_, StateRow>(
            r#"
            SELECT key, value_json, updated_at
            FROM _reactor_jobs.state
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn emit_event(&self, event: &NewEvent) -> Result<Event, JobsError> {
        let id = Uuid::now_v7();

        let row = sqlx::query_as::<_, EventRow>(
            r#"
            INSERT INTO _reactor_jobs.events (
                id, org_id, topic, payload_json, emitted_by_run_id
            ) VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(event.org_id)
        .bind(&event.topic)
        .bind(&event.payload_json)
        .bind(event.emitted_by_run_id)
        .fetch_one(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(row.into())
    }

    async fn consume_event(&self, id: EventId, run_id: RunId) -> Result<(), JobsError> {
        sqlx::query(
            r#"
            UPDATE _reactor_jobs.events
            SET consumed_by_run_id = $2, consumed_at = now()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn list_pending_events(&self, topic: &str, limit: u32) -> Result<Vec<Event>, JobsError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT * FROM _reactor_jobs.events
            WHERE topic = $1 AND consumed_at IS NULL
            ORDER BY created_at
            LIMIT $2
            "#,
        )
        .bind(topic)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_pending_events_for_org(
        &self,
        org_id: Uuid,
        limit: u32,
    ) -> Result<Vec<Event>, JobsError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT * FROM _reactor_jobs.events
            WHERE org_id = $1 AND consumed_at IS NULL
            ORDER BY created_at
            LIMIT $2
            "#,
        )
        .bind(org_id)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn move_to_dlq(&self, run_id: RunId, _reason: &str) -> Result<(), JobsError> {
        let id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO _reactor_jobs.dlq (
                id, run_id, job_id, org_id, payload_json, error_code, error_message, attempt
            )
            SELECT $1, r.id, r.job_id, r.org_id, r.payload_json, r.error_code, r.error_message, r.attempt
            FROM _reactor_jobs.runs r
            WHERE r.id = $2
            "#,
        )
        .bind(id)
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(())
    }

    async fn list_dlq(&self, job_id: JobId, limit: u32) -> Result<Vec<DlqEntry>, JobsError> {
        let rows = sqlx::query_as::<_, DlqRow>(
            r#"
            SELECT * FROM _reactor_jobs.dlq
            WHERE job_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(job_id)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn retry_from_dlq(&self, dlq_id: Uuid) -> Result<RunId, JobsError> {
        let run_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO _reactor_jobs.runs (
                id, job_id, org_id, trigger_kind, payload_json, max_attempts
            )
            SELECT $1, d.job_id, d.org_id, 'manual', d.payload_json,
                   (SELECT retry_max_attempts FROM _reactor_jobs.jobs WHERE id = d.job_id)
            FROM _reactor_jobs.dlq d
            WHERE d.id = $2
            "#,
        )
        .bind(run_id)
        .bind(dlq_id)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        // Delete from DLQ
        sqlx::query("DELETE FROM _reactor_jobs.dlq WHERE id = $1")
            .bind(dlq_id)
            .execute(&self.pool)
            .await
            .map_err(JobsError::Database)?;

        Ok(run_id)
    }

    async fn delete_dlq(&self, dlq_id: Uuid) -> Result<(), JobsError> {
        sqlx::query("DELETE FROM _reactor_jobs.dlq WHERE id = $1")
            .bind(dlq_id)
            .execute(&self.pool)
            .await
            .map_err(JobsError::Database)?;
        Ok(())
    }

    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), JobsError> {
        let id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO _reactor_jobs.audit_events (
                id, event_type, actor_user_id, actor_apikey_id, org_id, job_id, run_id, details, request_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(&event.event_type)
        .bind(event.actor_user_id)
        .bind(event.actor_apikey_id)
        .bind(event.org_id)
        .bind(event.job_id)
        .bind(event.run_id)
        .bind(&event.details)
        .bind(&event.request_id)
        .execute(&self.pool)
        .await
        .map_err(JobsError::Database)?;

        Ok(())
    }
}

// Row types for sqlx
#[derive(Debug, sqlx::FromRow)]
struct JobRow {
    id: Uuid,
    org_id: Uuid,
    name: String,
    function_name: String,
    description: Option<String>,
    retry_max_attempts: i32,
    retry_backoff: String,
    retry_initial_delay_ms: i32,
    retry_max_delay_ms: i32,
    max_concurrency: i32,
    timeout_ms: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<JobRow> for Job {
    fn from(r: JobRow) -> Self {
        Self {
            id: r.id,
            org_id: r.org_id,
            name: r.name,
            function_name: r.function_name,
            description: r.description,
            retry_max_attempts: r.retry_max_attempts,
            retry_backoff: r.retry_backoff,
            retry_initial_delay_ms: r.retry_initial_delay_ms,
            retry_max_delay_ms: r.retry_max_delay_ms,
            max_concurrency: r.max_concurrency,
            timeout_ms: r.timeout_ms,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct TriggerRow {
    id: Uuid,
    job_id: Uuid,
    kind: String,
    config_json: serde_json::Value,
    webhook_token: Option<String>,
    enabled: bool,
    last_triggered_at: Option<DateTime<Utc>>,
    next_trigger_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<TriggerRow> for Trigger {
    fn from(r: TriggerRow) -> Self {
        Self {
            id: r.id,
            job_id: r.job_id,
            kind: r.kind,
            config_json: r.config_json,
            webhook_token: r.webhook_token,
            enabled: r.enabled,
            last_triggered_at: r.last_triggered_at,
            next_trigger_at: r.next_trigger_at,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct RunRow {
    id: Uuid,
    job_id: Uuid,
    org_id: Uuid,
    trigger_id: Option<Uuid>,
    trigger_kind: String,
    status: String,
    payload_json: serde_json::Value,
    attempt: i32,
    max_attempts: i32,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    wakeup_at: Option<DateTime<Utc>>,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<RunRow> for Run {
    fn from(r: RunRow) -> Self {
        Self {
            id: r.id,
            job_id: r.job_id,
            org_id: r.org_id,
            trigger_id: r.trigger_id,
            trigger_kind: r.trigger_kind,
            status: r.status,
            payload_json: r.payload_json,
            attempt: r.attempt,
            max_attempts: r.max_attempts,
            started_at: r.started_at,
            finished_at: r.finished_at,
            wakeup_at: r.wakeup_at,
            error_code: r.error_code,
            error_message: r.error_message,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct StepRow {
    id: Uuid,
    run_id: Uuid,
    name: String,
    status: String,
    input_json: Option<serde_json::Value>,
    output_json: Option<serde_json::Value>,
    attempt: i32,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    error_message: Option<String>,
}

impl From<StepRow> for Step {
    fn from(r: StepRow) -> Self {
        Self {
            id: r.id,
            run_id: r.run_id,
            name: r.name,
            status: r.status,
            input_json: r.input_json,
            output_json: r.output_json,
            attempt: r.attempt,
            started_at: r.started_at,
            finished_at: r.finished_at,
            error_message: r.error_message,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct StateRow {
    key: String,
    value_json: serde_json::Value,
    updated_at: DateTime<Utc>,
}

impl From<StateRow> for StateEntry {
    fn from(r: StateRow) -> Self {
        Self {
            key: r.key,
            value_json: r.value_json,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    org_id: Uuid,
    topic: String,
    payload_json: serde_json::Value,
    emitted_by_run_id: Option<Uuid>,
    consumed_by_run_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
}

impl From<EventRow> for Event {
    fn from(r: EventRow) -> Self {
        Self {
            id: r.id,
            org_id: r.org_id,
            topic: r.topic,
            payload_json: r.payload_json,
            emitted_by_run_id: r.emitted_by_run_id,
            consumed_by_run_id: r.consumed_by_run_id,
            created_at: r.created_at,
            consumed_at: r.consumed_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DlqRow {
    id: Uuid,
    run_id: Uuid,
    job_id: Uuid,
    org_id: Uuid,
    payload_json: serde_json::Value,
    error_code: Option<String>,
    error_message: Option<String>,
    attempt: i32,
    created_at: DateTime<Utc>,
}

impl From<DlqRow> for DlqEntry {
    fn from(r: DlqRow) -> Self {
        Self {
            id: r.id,
            run_id: r.run_id,
            job_id: r.job_id,
            org_id: r.org_id,
            payload_json: r.payload_json,
            error_code: r.error_code,
            error_message: r.error_message,
            attempt: r.attempt,
            created_at: r.created_at,
        }
    }
}

/// Migration SQL for the jobs schema.
const MIGRATION_SQL: &str = include_str!("../../migrations/001_all.sql");
