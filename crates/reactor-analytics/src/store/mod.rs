//! Analytics store trait and implementations.

mod postgres;

pub use postgres::PgAnalyticsStore;

use crate::error::AnalyticsError;
use crate::query::{QueryRequest, QueryResult};
use crate::state::AnalyticsCtx;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Project record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Project creation params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreate {
    pub name: String,
}

/// Project key record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectKeyRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub org_id: Uuid,
    pub key_prefix: String,
    pub key_last4: String,
    pub name: String,
    pub sampling_rate: f64,
    pub allowed_origins: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Project key creation params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectKeyCreate {
    pub name: String,
    pub sampling_rate: Option<f64>,
    pub allowed_origins: Option<Vec<String>>,
}

/// Stored event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub id: Uuid,
    pub received_at: DateTime<Utc>,
    pub timestamp: DateTime<Utc>,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub event: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub referrer_host: Option<String>,
    pub utm_source: Option<String>,
    pub country: Option<String>,
    pub device_type: Option<String>,
    pub ingest_ip_h24: Option<String>,
    pub library_name: Option<String>,
    pub library_version: Option<String>,
    pub properties: serde_json::Value,
    pub context: serde_json::Value,
}

/// Write outcome for batch inserts.
#[derive(Debug, Clone, Serialize)]
pub struct WriteOutcome {
    pub accepted: usize,
    pub rejected: Vec<RejectReason>,
}

/// Rejection reason for an event in a batch.
#[derive(Debug, Clone, Serialize)]
pub struct RejectReason {
    pub index: usize,
    pub reason: String,
}

/// Erasure outcome.
#[derive(Debug, Clone, Serialize)]
pub struct EraseOutcome {
    pub rows_deleted: u64,
}

/// Analytics store trait.
#[async_trait]
pub trait AnalyticsStore: Send + Sync + 'static {
    /// Get the underlying connection pool (for health checks).
    fn pool(&self) -> &sqlx::PgPool;

    // --- Project & key management ---

    /// Create a new project.
    async fn create_project(
        &self,
        org_id: Uuid,
        project: ProjectCreate,
    ) -> Result<Project, AnalyticsError>;

    /// Get a project by ID.
    async fn get_project(&self, project_id: Uuid) -> Result<Option<Project>, AnalyticsError>;

    /// List projects for an organization.
    async fn list_projects(&self, org_id: Uuid) -> Result<Vec<Project>, AnalyticsError>;

    /// Delete a project (soft delete).
    async fn delete_project(&self, project_id: Uuid) -> Result<(), AnalyticsError>;

    /// Create a project key.
    async fn create_project_key(
        &self,
        project_id: Uuid,
        key_create: ProjectKeyCreate,
        key_hash: Vec<u8>,
        key_last4: String,
    ) -> Result<ProjectKeyRecord, AnalyticsError>;

    /// Look up a project key by hash.
    async fn lookup_project_key(
        &self,
        key_hash: &[u8],
    ) -> Result<Option<ProjectKeyRecord>, AnalyticsError>;

    /// List project keys.
    async fn list_project_keys(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<ProjectKeyRecord>, AnalyticsError>;

    /// Revoke a project key.
    async fn revoke_project_key(&self, key_id: Uuid) -> Result<(), AnalyticsError>;

    // --- Ingestion ---

    /// Write a batch of events.
    async fn write_events(&self, events: &[StoredEvent]) -> Result<WriteOutcome, AnalyticsError>;

    // --- Identity stitching ---

    /// Upsert an identity (link anonymous_id to user_id).
    async fn upsert_identity(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        anonymous_id: &str,
        user_id: &str,
        traits: &serde_json::Value,
    ) -> Result<(), AnalyticsError>;

    /// Alias an anonymous ID to a user ID.
    async fn alias(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        from_anonymous_id: &str,
        to_user_id: &str,
    ) -> Result<(), AnalyticsError>;

    /// Look up the user_id for an anonymous_id.
    async fn lookup_identity(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
    ) -> Result<Option<String>, AnalyticsError>;

    // --- Consent ---

    /// Check if an anonymous ID is tombstoned (opted out).
    async fn is_tombstoned(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
    ) -> Result<bool, AnalyticsError>;

    /// Create a consent tombstone (opt out).
    async fn create_tombstone(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
        reason: &str,
    ) -> Result<(), AnalyticsError>;

    /// Remove a consent tombstone (opt in).
    async fn remove_tombstone(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
    ) -> Result<(), AnalyticsError>;

    // --- Query ---

    /// Execute a query.
    async fn query(
        &self,
        ctx: &AnalyticsCtx,
        request: &QueryRequest,
    ) -> Result<QueryResult, AnalyticsError>;

    // --- GDPR ---

    /// Erase all events for a user.
    async fn erase_user(
        &self,
        project_id: Uuid,
        user_id: &str,
    ) -> Result<EraseOutcome, AnalyticsError>;

    /// Erase all events for an anonymous ID.
    async fn erase_anonymous(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
    ) -> Result<EraseOutcome, AnalyticsError>;

    /// Export all events for a user.
    async fn export_user(
        &self,
        project_id: Uuid,
        user_id: &str,
    ) -> Result<Vec<StoredEvent>, AnalyticsError>;

    // --- Quota ---

    /// Get the current month's event count for an organization.
    async fn current_month_event_count(&self, org_id: Uuid) -> Result<u64, AnalyticsError>;

    // --- Audit ---

    /// Write an audit event.
    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), AnalyticsError>;

    /// Write an erasure log entry.
    async fn write_erasure_log(&self, log: &ErasureLog) -> Result<(), AnalyticsError>;

    // --- Quota ---

    /// Get the monthly event count for an organization.
    async fn get_org_monthly_event_count(&self, org_id: Uuid) -> Result<u64, AnalyticsError>;

    // --- Query ---

    /// Execute an analytics query.
    async fn execute_query(
        &self,
        req: &crate::query::QueryRequest,
        ctx: &crate::state::AnalyticsCtx,
    ) -> Result<crate::query::QueryResult, AnalyticsError>;
}

/// Audit event for admin actions.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub actor_user_id: Option<Uuid>,
    pub actor_apikey_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub event_type: String,
    pub details: serde_json::Value,
    pub request_id: String,
}

/// Erasure log entry.
#[derive(Debug, Clone, Serialize)]
pub struct ErasureLog {
    pub project_id: Uuid,
    pub subject_kind: String,
    pub subject_id: String,
    pub rows_deleted: u64,
    pub actor_user_id: Option<Uuid>,
    pub request_id: String,
}
