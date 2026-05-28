//! Audit logging for storage operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::StorageError;

/// Audit event action types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// Object created or updated.
    ObjectPut,
    /// Object retrieved.
    ObjectGet,
    /// Object deleted.
    ObjectDelete,
    /// Object metadata retrieved.
    ObjectHead,
    /// Bucket created.
    BucketCreate,
    /// Bucket updated.
    BucketUpdate,
    /// Bucket deleted.
    BucketDelete,
    /// Multipart upload initiated.
    MultipartCreate,
    /// Part uploaded.
    MultipartPart,
    /// Multipart upload completed.
    MultipartComplete,
    /// Multipart upload aborted.
    MultipartAbort,
    /// Signed URL generated.
    SignedUrlCreate,
}

impl AuditAction {
    /// Get the action name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ObjectPut => "object_put",
            Self::ObjectGet => "object_get",
            Self::ObjectDelete => "object_delete",
            Self::ObjectHead => "object_head",
            Self::BucketCreate => "bucket_create",
            Self::BucketUpdate => "bucket_update",
            Self::BucketDelete => "bucket_delete",
            Self::MultipartCreate => "multipart_create",
            Self::MultipartPart => "multipart_part",
            Self::MultipartComplete => "multipart_complete",
            Self::MultipartAbort => "multipart_abort",
            Self::SignedUrlCreate => "signed_url_create",
        }
    }
}

/// Audit event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Event ID.
    pub id: Uuid,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Action performed.
    pub action: String,
    /// Organization ID.
    pub org_id: Option<Uuid>,
    /// User who performed the action.
    pub user_id: Option<Uuid>,
    /// Bucket ID (if applicable).
    pub bucket_id: Option<Uuid>,
    /// Object key (if applicable).
    pub object_key: Option<String>,
    /// Request ID for correlation.
    pub request_id: Option<String>,
    /// IP address of the requester.
    pub ip_address: Option<String>,
    /// User agent of the requester.
    pub user_agent: Option<String>,
    /// Additional metadata.
    pub metadata: serde_json::Value,
}

/// Audit event builder.
#[derive(Debug, Default)]
pub struct AuditEventBuilder {
    action: Option<AuditAction>,
    org_id: Option<Uuid>,
    user_id: Option<Uuid>,
    bucket_id: Option<Uuid>,
    object_key: Option<String>,
    request_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    metadata: serde_json::Value,
}

impl AuditEventBuilder {
    /// Create a new audit event builder.
    pub fn new(action: AuditAction) -> Self {
        Self {
            action: Some(action),
            metadata: serde_json::json!({}),
            ..Default::default()
        }
    }

    /// Set the organization ID.
    pub fn org_id(mut self, org_id: Uuid) -> Self {
        self.org_id = Some(org_id);
        self
    }

    /// Set the user ID.
    pub fn user_id(mut self, user_id: Option<Uuid>) -> Self {
        self.user_id = user_id;
        self
    }

    /// Set the bucket ID.
    pub fn bucket_id(mut self, bucket_id: Uuid) -> Self {
        self.bucket_id = Some(bucket_id);
        self
    }

    /// Set the object key.
    pub fn object_key(mut self, key: impl Into<String>) -> Self {
        self.object_key = Some(key.into());
        self
    }

    /// Set the request ID.
    pub fn request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Set the IP address.
    pub fn ip_address(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Set the user agent.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// Set additional metadata.
    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Build and record the audit event.
    pub async fn record(self, pool: &PgPool) -> Result<Uuid, StorageError> {
        let id = Uuid::now_v7();
        let action = self.action.map(|a| a.as_str().to_string()).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO _reactor_storage.audit_events
                (id, action, org_id, user_id, bucket_id, object_key, request_id, ip_address, user_agent, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(id)
        .bind(&action)
        .bind(self.org_id)
        .bind(self.user_id)
        .bind(self.bucket_id)
        .bind(&self.object_key)
        .bind(&self.request_id)
        .bind(&self.ip_address)
        .bind(&self.user_agent)
        .bind(&self.metadata)
        .execute(pool)
        .await?;

        Ok(id)
    }
}

/// List recent audit events.
pub async fn list_audit_events(
    pool: &PgPool,
    org_id: Option<Uuid>,
    bucket_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<AuditEvent>, StorageError> {
    use sqlx::FromRow;

    #[derive(FromRow)]
    struct AuditEventRow {
        id: Uuid,
        timestamp: DateTime<Utc>,
        action: String,
        org_id: Option<Uuid>,
        user_id: Option<Uuid>,
        bucket_id: Option<Uuid>,
        object_key: Option<String>,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        metadata: serde_json::Value,
    }

    let rows: Vec<AuditEventRow> = if let Some(oid) = org_id {
        if let Some(bid) = bucket_id {
            sqlx::query_as::<_, AuditEventRow>(
                r#"
                SELECT id, timestamp, action, org_id, user_id, bucket_id, object_key, 
                       request_id, ip_address, user_agent, metadata
                FROM _reactor_storage.audit_events
                WHERE org_id = $1 AND bucket_id = $2
                ORDER BY timestamp DESC
                LIMIT $3
                "#,
            )
            .bind(oid)
            .bind(bid)
            .bind(limit)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, AuditEventRow>(
                r#"
                SELECT id, timestamp, action, org_id, user_id, bucket_id, object_key, 
                       request_id, ip_address, user_agent, metadata
                FROM _reactor_storage.audit_events
                WHERE org_id = $1
                ORDER BY timestamp DESC
                LIMIT $2
                "#,
            )
            .bind(oid)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    } else {
        sqlx::query_as::<_, AuditEventRow>(
            r#"
            SELECT id, timestamp, action, org_id, user_id, bucket_id, object_key, 
                   request_id, ip_address, user_agent, metadata
            FROM _reactor_storage.audit_events
            ORDER BY timestamp DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    // Convert rows to AuditEvent
    let events = rows
        .into_iter()
        .map(|row| AuditEvent {
            id: row.id,
            timestamp: row.timestamp,
            action: row.action,
            org_id: row.org_id,
            user_id: row.user_id,
            bucket_id: row.bucket_id,
            object_key: row.object_key,
            request_id: row.request_id,
            ip_address: row.ip_address,
            user_agent: row.user_agent,
            metadata: row.metadata,
        })
        .collect();

    Ok(events)
}
