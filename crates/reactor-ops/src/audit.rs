//! Audit logging for ops operations.

use reactor_core::id::UserId;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// An ops audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsAuditEntry {
    /// Actor user ID.
    pub actor_user_id: Option<UserId>,
    /// Actor IP address.
    pub actor_ip: Option<String>,
    /// Actor user agent.
    pub actor_user_agent: Option<String>,
    /// Action performed (e.g., "deploy", "project.create").
    pub action: String,
    /// Scope used for authorization.
    pub scope_used: Option<String>,
    /// Type of resource affected.
    pub resource_type: Option<String>,
    /// ID of the affected resource.
    pub resource_id: Option<String>,
    /// Status of the operation.
    pub status: AuditStatus,
    /// Additional details.
    pub details: serde_json::Value,
    /// Whether step-up authentication was used.
    pub step_up_used: bool,
}

/// Status of an audited operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditStatus {
    /// Operation succeeded.
    Success,
    /// Operation was denied.
    Denied,
    /// Operation failed with an error.
    Error,
}

impl std::fmt::Display for AuditStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditStatus::Success => write!(f, "success"),
            AuditStatus::Denied => write!(f, "denied"),
            AuditStatus::Error => write!(f, "error"),
        }
    }
}

/// Audit logger for ops operations.
#[derive(Clone)]
pub struct AuditLogger {
    pool: PgPool,
}

impl AuditLogger {
    /// Create a new audit logger.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Log an audit entry.
    pub async fn log(&self, entry: &OpsAuditEntry) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO reactor_auth.ops_audit_log 
                (actor_user_id, actor_ip, actor_user_agent, action, scope_used, 
                 resource_type, resource_id, status, details, step_up_used)
            VALUES 
                ($1, $2::inet, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(entry.actor_user_id.map(|u| u.into_uuid()))
        .bind(&entry.actor_ip)
        .bind(&entry.actor_user_agent)
        .bind(&entry.action)
        .bind(&entry.scope_used)
        .bind(&entry.resource_type)
        .bind(&entry.resource_id)
        .bind(entry.status.to_string())
        .bind(&entry.details)
        .bind(entry.step_up_used)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Query recent audit entries.
    pub async fn query_recent(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEntryRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, AuditEntryRow>(
            r#"
            SELECT id, ts, actor_user_id, actor_ip, actor_user_agent, action, 
                   scope_used, resource_type, resource_id, status, details, step_up_used
            FROM reactor_auth.ops_audit_log
            ORDER BY ts DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Query audit entries by actor.
    pub async fn query_by_actor(
        &self,
        actor_user_id: &UserId,
        limit: i64,
    ) -> Result<Vec<AuditEntryRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, AuditEntryRow>(
            r#"
            SELECT id, ts, actor_user_id, actor_ip, actor_user_agent, action, 
                   scope_used, resource_type, resource_id, status, details, step_up_used
            FROM reactor_auth.ops_audit_log
            WHERE actor_user_id = $1
            ORDER BY ts DESC
            LIMIT $2
            "#,
        )
        .bind(actor_user_id.as_uuid())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Cleanup old audit entries based on retention period.
    pub async fn cleanup(&self, retention_days: u32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.ops_audit_log
            WHERE ts < NOW() - INTERVAL '1 day' * $1
            "#,
        )
        .bind(retention_days as i32)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

/// A row from the audit log.
#[derive(Debug, sqlx::FromRow)]
pub struct AuditEntryRow {
    /// Entry ID.
    pub id: i64,
    /// Timestamp.
    pub ts: chrono::DateTime<chrono::Utc>,
    /// Actor user ID.
    pub actor_user_id: Option<uuid::Uuid>,
    /// Actor IP.
    pub actor_ip: Option<String>,
    /// Actor user agent.
    pub actor_user_agent: Option<String>,
    /// Action.
    pub action: String,
    /// Scope used.
    pub scope_used: Option<String>,
    /// Resource type.
    pub resource_type: Option<String>,
    /// Resource ID.
    pub resource_id: Option<String>,
    /// Status.
    pub status: String,
    /// Details.
    pub details: serde_json::Value,
    /// Whether step-up was used.
    pub step_up_used: bool,
}
