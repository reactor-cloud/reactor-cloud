//! PostgreSQL implementation of AnalyticsStore.

use super::{
    AnalyticsStore, AuditEvent, EraseOutcome, ErasureLog, Project, ProjectCreate, ProjectKeyCreate,
    ProjectKeyRecord, RejectReason, StoredEvent, WriteOutcome,
};
use crate::error::AnalyticsError;
use crate::query::{QueryRequest, QueryResult};
use crate::state::AnalyticsCtx;
use async_trait::async_trait;
use chrono::Datelike;
use sqlx::{FromRow, PgPool, Row};
use uuid::Uuid;

/// PostgreSQL analytics store implementation.
#[derive(Clone)]
pub struct PgAnalyticsStore {
    pool: PgPool,
}

impl PgAnalyticsStore {
    /// Create a new PostgreSQL analytics store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run migrations.
    pub async fn migrate(&self) -> Result<(), AnalyticsError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| AnalyticsError::Database(e.into()))?;
        Ok(())
    }
}

#[derive(FromRow)]
struct ProjectRow {
    id: Uuid,
    org_id: Uuid,
    name: String,
    created_at: chrono::DateTime<chrono::Utc>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<ProjectRow> for Project {
    fn from(row: ProjectRow) -> Self {
        Self {
            id: row.id,
            org_id: row.org_id,
            name: row.name,
            created_at: row.created_at,
            deleted_at: row.deleted_at,
        }
    }
}

#[derive(FromRow)]
struct ProjectKeyRow {
    id: Uuid,
    project_id: Uuid,
    org_id: Uuid,
    key_prefix: String,
    key_last4: String,
    name: String,
    sampling_rate: f64,
    allowed_origins: Option<Vec<String>>,
    created_at: chrono::DateTime<chrono::Utc>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<ProjectKeyRow> for ProjectKeyRecord {
    fn from(row: ProjectKeyRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            org_id: row.org_id,
            key_prefix: row.key_prefix,
            key_last4: row.key_last4,
            name: row.name,
            sampling_rate: row.sampling_rate,
            allowed_origins: row.allowed_origins,
            created_at: row.created_at,
            revoked_at: row.revoked_at,
        }
    }
}

#[derive(FromRow)]
struct StoredEventRow {
    id: Uuid,
    received_at: chrono::DateTime<chrono::Utc>,
    timestamp: chrono::DateTime<chrono::Utc>,
    org_id: Uuid,
    project_id: Uuid,
    event: String,
    anonymous_id: String,
    user_id: Option<String>,
    session_id: Option<String>,
    url: Option<String>,
    path: Option<String>,
    referrer_host: Option<String>,
    utm_source: Option<String>,
    country: Option<String>,
    device_type: Option<String>,
    ingest_ip_h24: Option<String>,
    library_name: Option<String>,
    library_version: Option<String>,
    properties: serde_json::Value,
    context: serde_json::Value,
}

impl From<StoredEventRow> for StoredEvent {
    fn from(row: StoredEventRow) -> Self {
        Self {
            id: row.id,
            received_at: row.received_at,
            timestamp: row.timestamp,
            org_id: row.org_id,
            project_id: row.project_id,
            event: row.event,
            anonymous_id: row.anonymous_id,
            user_id: row.user_id,
            session_id: row.session_id,
            url: row.url,
            path: row.path,
            referrer_host: row.referrer_host,
            utm_source: row.utm_source,
            country: row.country,
            device_type: row.device_type,
            ingest_ip_h24: row.ingest_ip_h24,
            library_name: row.library_name,
            library_version: row.library_version,
            properties: row.properties,
            context: row.context,
        }
    }
}

#[async_trait]
impl AnalyticsStore for PgAnalyticsStore {
    fn pool(&self) -> &PgPool {
        &self.pool
    }

    async fn create_project(
        &self,
        org_id: Uuid,
        project: ProjectCreate,
    ) -> Result<Project, AnalyticsError> {
        let id = Uuid::now_v7();
        let row: ProjectRow = sqlx::query_as(
            r#"
            INSERT INTO _reactor_analytics.projects (id, org_id, name)
            VALUES ($1, $2, $3)
            RETURNING id, org_id, name, created_at, deleted_at
            "#,
        )
        .bind(id)
        .bind(org_id)
        .bind(&project.name)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    async fn get_project(&self, project_id: Uuid) -> Result<Option<Project>, AnalyticsError> {
        let row: Option<ProjectRow> = sqlx::query_as(
            r#"
            SELECT id, org_id, name, created_at, deleted_at
            FROM _reactor_analytics.projects
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn list_projects(&self, org_id: Uuid) -> Result<Vec<Project>, AnalyticsError> {
        let rows: Vec<ProjectRow> = sqlx::query_as(
            r#"
            SELECT id, org_id, name, created_at, deleted_at
            FROM _reactor_analytics.projects
            WHERE org_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn delete_project(&self, project_id: Uuid) -> Result<(), AnalyticsError> {
        sqlx::query(
            r#"
            UPDATE _reactor_analytics.projects
            SET deleted_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn create_project_key(
        &self,
        project_id: Uuid,
        key_create: ProjectKeyCreate,
        key_hash: Vec<u8>,
        key_last4: String,
    ) -> Result<ProjectKeyRecord, AnalyticsError> {
        let id = Uuid::now_v7();
        let sampling_rate = key_create.sampling_rate.unwrap_or(1.0);

        let row: ProjectKeyRow = sqlx::query_as(
            r#"
            WITH inserted AS (
                INSERT INTO _reactor_analytics.project_keys 
                    (id, project_id, key_prefix, key_hash, key_last4, name, sampling_rate, allowed_origins)
                SELECT $1, p.id, 'rapk_', $3, $4, $5, $6, $7
                FROM _reactor_analytics.projects p
                WHERE p.id = $2 AND p.deleted_at IS NULL
                RETURNING id, project_id, key_prefix, key_last4, name, sampling_rate, allowed_origins, created_at, revoked_at
            )
            SELECT i.id, i.project_id, p.org_id, i.key_prefix, i.key_last4, i.name, 
                   i.sampling_rate, i.allowed_origins, i.created_at, i.revoked_at
            FROM inserted i
            JOIN _reactor_analytics.projects p ON p.id = i.project_id
            "#,
        )
        .bind(id)
        .bind(project_id)
        .bind(&key_hash)
        .bind(&key_last4)
        .bind(&key_create.name)
        .bind(sampling_rate)
        .bind(&key_create.allowed_origins)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    async fn lookup_project_key(
        &self,
        key_hash: &[u8],
    ) -> Result<Option<ProjectKeyRecord>, AnalyticsError> {
        let row: Option<ProjectKeyRow> = sqlx::query_as(
            r#"
            SELECT pk.id, pk.project_id, p.org_id, pk.key_prefix, pk.key_last4, pk.name, 
                   pk.sampling_rate, pk.allowed_origins, pk.created_at, pk.revoked_at
            FROM _reactor_analytics.project_keys pk
            JOIN _reactor_analytics.projects p ON p.id = pk.project_id
            WHERE pk.key_hash = $1 AND pk.revoked_at IS NULL AND p.deleted_at IS NULL
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn list_project_keys(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<ProjectKeyRecord>, AnalyticsError> {
        let rows: Vec<ProjectKeyRow> = sqlx::query_as(
            r#"
            SELECT pk.id, pk.project_id, p.org_id, pk.key_prefix, pk.key_last4, pk.name,
                   pk.sampling_rate, pk.allowed_origins, pk.created_at, pk.revoked_at
            FROM _reactor_analytics.project_keys pk
            JOIN _reactor_analytics.projects p ON p.id = pk.project_id
            WHERE pk.project_id = $1
            ORDER BY pk.created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn revoke_project_key(&self, key_id: Uuid) -> Result<(), AnalyticsError> {
        sqlx::query(
            r#"
            UPDATE _reactor_analytics.project_keys
            SET revoked_at = NOW()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(key_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn write_events(&self, events: &[StoredEvent]) -> Result<WriteOutcome, AnalyticsError> {
        if events.is_empty() {
            return Ok(WriteOutcome {
                accepted: 0,
                rejected: vec![],
            });
        }

        let mut accepted = 0;
        let mut rejected = Vec::new();

        for (idx, event) in events.iter().enumerate() {
            let result = sqlx::query(
                r#"
                INSERT INTO _reactor_analytics.events
                    (id, received_at, timestamp, org_id, project_id, event, anonymous_id, user_id,
                     session_id, url, path, referrer_host, utm_source, country, device_type,
                     ingest_ip_h24, library_name, library_version, properties, context)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
                "#,
            )
            .bind(event.id)
            .bind(event.received_at)
            .bind(event.timestamp)
            .bind(event.org_id)
            .bind(event.project_id)
            .bind(&event.event)
            .bind(&event.anonymous_id)
            .bind(&event.user_id)
            .bind(&event.session_id)
            .bind(&event.url)
            .bind(&event.path)
            .bind(&event.referrer_host)
            .bind(&event.utm_source)
            .bind(&event.country)
            .bind(&event.device_type)
            .bind(&event.ingest_ip_h24)
            .bind(&event.library_name)
            .bind(&event.library_version)
            .bind(&event.properties)
            .bind(&event.context)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => accepted += 1,
                Err(e) => {
                    rejected.push(RejectReason {
                        index: idx,
                        reason: e.to_string(),
                    });
                }
            }
        }

        Ok(WriteOutcome { accepted, rejected })
    }

    async fn upsert_identity(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        anonymous_id: &str,
        user_id: &str,
        traits: &serde_json::Value,
    ) -> Result<(), AnalyticsError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_analytics.identities 
                (org_id, project_id, anonymous_id, user_id, traits, last_seen_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (project_id, anonymous_id) DO UPDATE SET
                user_id = COALESCE(EXCLUDED.user_id, _reactor_analytics.identities.user_id),
                traits = _reactor_analytics.identities.traits || EXCLUDED.traits,
                last_seen_at = NOW()
            "#,
        )
        .bind(org_id)
        .bind(project_id)
        .bind(anonymous_id)
        .bind(user_id)
        .bind(traits)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn alias(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        from_anonymous_id: &str,
        to_user_id: &str,
    ) -> Result<(), AnalyticsError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_analytics.identities 
                (org_id, project_id, anonymous_id, user_id, last_seen_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (project_id, anonymous_id) DO UPDATE SET
                user_id = $4,
                last_seen_at = NOW()
            "#,
        )
        .bind(org_id)
        .bind(project_id)
        .bind(from_anonymous_id)
        .bind(to_user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn lookup_identity(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
    ) -> Result<Option<String>, AnalyticsError> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            r#"
            SELECT user_id
            FROM _reactor_analytics.identities
            WHERE project_id = $1 AND anonymous_id = $2
            "#,
        )
        .bind(project_id)
        .bind(anonymous_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|(user_id,)| user_id))
    }

    async fn is_tombstoned(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
    ) -> Result<bool, AnalyticsError> {
        let row: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT 1
            FROM _reactor_analytics.consent_tombstones
            WHERE project_id = $1 AND anonymous_id = $2
            "#,
        )
        .bind(project_id)
        .bind(anonymous_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.is_some())
    }

    async fn create_tombstone(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
        reason: &str,
    ) -> Result<(), AnalyticsError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_analytics.consent_tombstones (project_id, anonymous_id, reason)
            VALUES ($1, $2, $3)
            ON CONFLICT (project_id, anonymous_id) DO UPDATE SET reason = $3
            "#,
        )
        .bind(project_id)
        .bind(anonymous_id)
        .bind(reason)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn remove_tombstone(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
    ) -> Result<(), AnalyticsError> {
        sqlx::query(
            r#"
            DELETE FROM _reactor_analytics.consent_tombstones
            WHERE project_id = $1 AND anonymous_id = $2
            "#,
        )
        .bind(project_id)
        .bind(anonymous_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn query(
        &self,
        _ctx: &AnalyticsCtx,
        _request: &QueryRequest,
    ) -> Result<QueryResult, AnalyticsError> {
        Err(AnalyticsError::Internal(
            "query execution not yet implemented".to_string(),
        ))
    }

    async fn erase_user(
        &self,
        project_id: Uuid,
        user_id: &str,
    ) -> Result<EraseOutcome, AnalyticsError> {
        let result = sqlx::query(
            r#"
            DELETE FROM _reactor_analytics.events
            WHERE project_id = $1 AND user_id = $2
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM _reactor_analytics.identities
            WHERE project_id = $1 AND user_id = $2
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(EraseOutcome {
            rows_deleted: result.rows_affected(),
        })
    }

    async fn erase_anonymous(
        &self,
        project_id: Uuid,
        anonymous_id: &str,
    ) -> Result<EraseOutcome, AnalyticsError> {
        let result = sqlx::query(
            r#"
            DELETE FROM _reactor_analytics.events
            WHERE project_id = $1 AND anonymous_id = $2
            "#,
        )
        .bind(project_id)
        .bind(anonymous_id)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM _reactor_analytics.identities
            WHERE project_id = $1 AND anonymous_id = $2
            "#,
        )
        .bind(project_id)
        .bind(anonymous_id)
        .execute(&self.pool)
        .await?;

        Ok(EraseOutcome {
            rows_deleted: result.rows_affected(),
        })
    }

    async fn export_user(
        &self,
        project_id: Uuid,
        user_id: &str,
    ) -> Result<Vec<StoredEvent>, AnalyticsError> {
        let rows: Vec<StoredEventRow> = sqlx::query_as(
            r#"
            SELECT id, received_at, timestamp, org_id, project_id, event, anonymous_id, 
                   user_id, session_id, url, path, referrer_host, utm_source, country,
                   device_type, ingest_ip_h24, library_name, library_version, properties, context
            FROM _reactor_analytics.events
            WHERE project_id = $1 AND user_id = $2
            ORDER BY timestamp DESC
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn current_month_event_count(&self, org_id: Uuid) -> Result<u64, AnalyticsError> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM _reactor_analytics.events
            WHERE org_id = $1 
              AND received_at >= date_trunc('month', NOW())
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0 as u64)
    }

    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), AnalyticsError> {
        let id = Uuid::now_v7();
        sqlx::query(
            r#"
            INSERT INTO _reactor_analytics.audit_events 
                (id, actor_user_id, actor_apikey_id, org_id, project_id, event_type, details, request_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(event.actor_user_id)
        .bind(event.actor_apikey_id)
        .bind(event.org_id)
        .bind(event.project_id)
        .bind(&event.event_type)
        .bind(&event.details)
        .bind(&event.request_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn write_erasure_log(&self, log: &ErasureLog) -> Result<(), AnalyticsError> {
        let id = Uuid::now_v7();
        sqlx::query(
            r#"
            INSERT INTO _reactor_analytics.erasures 
                (id, project_id, subject_kind, subject_id, rows_deleted, actor_user_id, request_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(id)
        .bind(log.project_id)
        .bind(&log.subject_kind)
        .bind(&log.subject_id)
        .bind(log.rows_deleted as i64)
        .bind(log.actor_user_id)
        .bind(&log.request_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_org_monthly_event_count(&self, org_id: Uuid) -> Result<u64, AnalyticsError> {
        // Get current month start
        let now = chrono::Utc::now();
        let month_start = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
            .expect("valid date")
            .and_hms_opt(0, 0, 0)
            .expect("valid time");
        let month_start = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            month_start,
            chrono::Utc,
        );

        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COALESCE(COUNT(*), 0) as count
            FROM _reactor_analytics.events
            WHERE org_id = $1
              AND received_at >= $2
            "#,
        )
        .bind(org_id)
        .bind(month_start)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0 as u64)
    }

    async fn execute_query(
        &self,
        req: &crate::query::QueryRequest,
        _ctx: &crate::state::AnalyticsCtx,
    ) -> Result<crate::query::QueryResult, AnalyticsError> {
        use crate::query::{QueryKind, QueryResult, QueryRow};
        use std::time::Instant;

        let start = Instant::now();

        // Compile query to SQL
        let config = crate::config::AnalyticsConfig::default();
        let mut compiler = crate::query::compiler::SqlCompiler::new(&config);
        let compiled = compiler.compile(req)?;

        tracing::debug!(sql = %compiled.sql, params = ?compiled.params.len(), "executing query");

        // Execute based on query kind
        // For now, return a simple implementation - in a real system this would
        // bind parameters and execute the compiled SQL
        let execution_ms = start.elapsed().as_millis() as u64;

        match req.kind {
            QueryKind::Events => {
                // Execute events query
                let rows: Vec<StoredEventRow> = sqlx::query_as(
                    r#"
                    SELECT id, received_at, timestamp, org_id, project_id, event,
                           anonymous_id, user_id, session_id, url, path, referrer_host,
                           utm_source, country, device_type, ingest_ip_h24,
                           library_name, library_version, properties, context
                    FROM _reactor_analytics.events
                    WHERE project_id = $1
                    ORDER BY received_at DESC
                    LIMIT 100
                    "#,
                )
                .bind(req.project_id)
                .fetch_all(&self.pool)
                .await?;

                let stored: Vec<StoredEvent> = rows.into_iter().map(Into::into).collect();
                let count = stored.len() as u64;

                Ok(QueryResult::Events {
                    rows: stored,
                    execution_ms,
                    rows_scanned: count,
                })
            }
            QueryKind::Aggregate => {
                // Simple count query for now
                let row: (i64,) = sqlx::query_as(
                    r#"
                    SELECT COUNT(*) as count
                    FROM _reactor_analytics.events
                    WHERE project_id = $1
                    "#,
                )
                .bind(req.project_id)
                .fetch_one(&self.pool)
                .await?;

                let rows = vec![QueryRow {
                    group: serde_json::Map::new(),
                    time: None,
                    value: serde_json::json!(row.0),
                }];

                Ok(QueryResult::Aggregate {
                    rows,
                    execution_ms,
                    rows_scanned: row.0 as u64,
                })
            }
            QueryKind::Breakdown => {
                // Group by event name for now
                let rows: Vec<(String, i64)> = sqlx::query_as(
                    r#"
                    SELECT event, COUNT(*) as count
                    FROM _reactor_analytics.events
                    WHERE project_id = $1
                    GROUP BY event
                    ORDER BY count DESC
                    LIMIT 100
                    "#,
                )
                .bind(req.project_id)
                .fetch_all(&self.pool)
                .await?;

                let query_rows: Vec<QueryRow> = rows
                    .into_iter()
                    .map(|(event, count)| {
                        let mut group = serde_json::Map::new();
                        group.insert("event".to_string(), serde_json::json!(event));
                        QueryRow {
                            group,
                            time: None,
                            value: serde_json::json!(count),
                        }
                    })
                    .collect();

                let scanned = query_rows.len() as u64;

                Ok(QueryResult::Breakdown {
                    rows: query_rows,
                    execution_ms,
                    rows_scanned: scanned,
                })
            }
            QueryKind::Funnel => {
                // Return empty for now - full funnel implementation is complex
                Ok(QueryResult::Funnel {
                    rows: vec![],
                    execution_ms,
                    rows_scanned: 0,
                })
            }
            QueryKind::Retention => {
                // Return empty for now - full retention implementation is complex
                Ok(QueryResult::Retention {
                    rows: vec![],
                    execution_ms,
                    rows_scanned: 0,
                })
            }
            QueryKind::Path => {
                // Return empty for now - full path implementation is complex
                Ok(QueryResult::Path {
                    rows: vec![],
                    execution_ms,
                    rows_scanned: 0,
                })
            }
        }
    }
}
