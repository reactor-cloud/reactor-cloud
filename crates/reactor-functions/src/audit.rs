//! Audit and invocation recording.
//!
//! Non-blocking audit event and invocation recording for observability.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

/// Audit event types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    FunctionCreate,
    FunctionDelete,
    DeploymentCreate,
    DeploymentPromote,
    DeploymentRollback,
    DeploymentFail,
    DeploymentDestroy,
    EnvUpsert,
    EnvDelete,
    PolicyCreate,
    PolicyDelete,
    PolicyBypass,
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditAction::FunctionCreate => write!(f, "function.create"),
            AuditAction::FunctionDelete => write!(f, "function.delete"),
            AuditAction::DeploymentCreate => write!(f, "deployment.create"),
            AuditAction::DeploymentPromote => write!(f, "deployment.promote"),
            AuditAction::DeploymentRollback => write!(f, "deployment.rollback"),
            AuditAction::DeploymentFail => write!(f, "deployment.fail"),
            AuditAction::DeploymentDestroy => write!(f, "deployment.destroy"),
            AuditAction::EnvUpsert => write!(f, "env.upsert"),
            AuditAction::EnvDelete => write!(f, "env.delete"),
            AuditAction::PolicyCreate => write!(f, "policy.create"),
            AuditAction::PolicyDelete => write!(f, "policy.delete"),
            AuditAction::PolicyBypass => write!(f, "policy.bypass"),
        }
    }
}

/// Audit event to be recorded.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Organization ID.
    pub org_id: Uuid,
    /// User ID (if available).
    pub user_id: Option<Uuid>,
    /// Audit action type.
    pub action: AuditAction,
    /// Function name.
    pub function_name: String,
    /// Deployment ID (if applicable).
    pub deployment_id: Option<Uuid>,
    /// Additional details (JSON).
    pub details: Option<serde_json::Value>,
}

/// Write an audit event to the database (non-blocking).
///
/// This spawns a background task to avoid blocking the request handler.
pub fn record_audit_event(pool: PgPool, event: AuditEvent) {
    tokio::spawn(async move {
        if let Err(e) = write_audit_event_inner(&pool, &event).await {
            tracing::error!(
                action = %event.action,
                function = %event.function_name,
                error = %e,
                "failed to record audit event"
            );
        }
    });
}

async fn write_audit_event_inner(
    pool: &PgPool,
    event: &AuditEvent,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO _reactor_functions.audit_events
            (org_id, user_id, action, function_name, deployment_id, details)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(event.org_id)
    .bind(event.user_id)
    .bind(event.action.to_string())
    .bind(&event.function_name)
    .bind(event.deployment_id)
    .bind(&event.details)
    .execute(pool)
    .await?;

    Ok(())
}

/// Invocation status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationStatus {
    /// Invocation completed successfully.
    Success,
    /// Invocation failed with an error.
    Error,
    /// Invocation timed out.
    Timeout,
    /// Invocation was cancelled.
    Cancelled,
}

impl std::fmt::Display for InvocationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvocationStatus::Success => write!(f, "success"),
            InvocationStatus::Error => write!(f, "error"),
            InvocationStatus::Timeout => write!(f, "timeout"),
            InvocationStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Invocation record to be persisted.
#[derive(Debug, Clone)]
pub struct InvocationRecord {
    /// Unique request ID.
    pub request_id: Uuid,
    /// Organization ID.
    pub org_id: Uuid,
    /// Function ID.
    pub function_id: Uuid,
    /// Deployment ID.
    pub deployment_id: Uuid,
    /// Invocation status.
    pub status: InvocationStatus,
    /// Invocation duration.
    pub duration: Duration,
    /// Request body size in bytes.
    pub request_bytes: u64,
    /// Response body size in bytes.
    pub response_bytes: u64,
    /// Whether this was a cold start.
    pub cold_start: bool,
    /// Error code if invocation failed.
    pub error_code: Option<String>,
}

/// Record an invocation to the database (non-blocking).
///
/// This spawns a background task to avoid blocking the response.
pub fn record_invocation(pool: PgPool, record: InvocationRecord) {
    tokio::spawn(async move {
        if let Err(e) = write_invocation_inner(&pool, &record).await {
            tracing::error!(
                request_id = %record.request_id,
                function_id = %record.function_id,
                error = %e,
                "failed to record invocation"
            );
        }
    });
}

async fn write_invocation_inner(
    pool: &PgPool,
    record: &InvocationRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO _reactor_functions.invocations
            (id, org_id, function_id, deployment_id, status, duration_ms, 
             request_bytes, response_bytes, cold_start, error_code)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(record.request_id)
    .bind(record.org_id)
    .bind(record.function_id)
    .bind(record.deployment_id)
    .bind(record.status.to_string())
    .bind(record.duration.as_millis() as i64)
    .bind(record.request_bytes as i64)
    .bind(record.response_bytes as i64)
    .bind(record.cold_start)
    .bind(&record.error_code)
    .execute(pool)
    .await?;

    Ok(())
}
