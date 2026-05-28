//! Postgres-backed cache implementation.
//!
//! Uses `FOR UPDATE SKIP LOCKED` for efficient queue dequeue operations.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

use crate::error::CacheError;
use crate::kv::KvOperations;
use crate::queue::{QueueItem, QueueOperations};

/// Postgres-backed cache backend.
#[derive(Debug, Clone)]
pub struct PostgresBackend {
    pool: PgPool,
}

impl PostgresBackend {
    /// Create a new Postgres backend.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run migrations for the cache schema.
    pub async fn migrate(&self) -> Result<(), CacheError> {
        sqlx::raw_sql(MIGRATION_SQL)
            .execute(&self.pool)
            .await
            .map_err(CacheError::Database)?;
        Ok(())
    }

    /// Health check - verify database connectivity.
    pub async fn health_check(&self) -> Result<(), CacheError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(CacheError::Database)?;
        Ok(())
    }
}

#[async_trait]
impl QueueOperations for PostgresBackend {
    async fn enqueue(
        &self,
        queue: &str,
        item: &[u8],
        delay: Option<Duration>,
    ) -> Result<String, CacheError> {
        let id = Uuid::now_v7();
        let visible_at = match delay {
            Some(d) => Utc::now() + chrono::Duration::from_std(d).unwrap_or_default(),
            None => Utc::now(),
        };

        sqlx::query(
            r#"
            INSERT INTO _reactor_cache.queue (id, queue_name, data, visible_at, attempt)
            VALUES ($1, $2, $3, $4, 0)
            "#,
        )
        .bind(id)
        .bind(queue)
        .bind(item)
        .bind(visible_at)
        .execute(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        Ok(id.to_string())
    }

    async fn dequeue(
        &self,
        queue: &str,
        count: u32,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueueItem>, CacheError> {
        let timeout_secs = visibility_timeout.as_secs() as i64;
        let receipt_prefix = Uuid::now_v7().to_string();

        // Use FOR UPDATE SKIP LOCKED to dequeue without blocking
        let rows: Vec<QueueRow> = sqlx::query_as(
            r#"
            WITH dequeued AS (
                SELECT id
                FROM _reactor_cache.queue
                WHERE queue_name = $1
                  AND visible_at <= now()
                  AND receipt IS NULL
                ORDER BY visible_at
                LIMIT $2
                FOR UPDATE SKIP LOCKED
            )
            UPDATE _reactor_cache.queue q
            SET receipt = $3 || '-' || q.id::text,
                visible_at = now() + ($4 || ' seconds')::interval,
                attempt = attempt + 1
            FROM dequeued d
            WHERE q.id = d.id
            RETURNING q.id, q.receipt, q.data, q.created_at as enqueued_at, q.attempt
            "#,
        )
        .bind(queue)
        .bind(count as i32)
        .bind(&receipt_prefix)
        .bind(timeout_secs.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        Ok(rows
            .into_iter()
            .map(|r| QueueItem {
                id: r.id.to_string(),
                receipt: r.receipt.unwrap_or_default(),
                data: r.data,
                enqueued_at: r.enqueued_at,
                attempt: r.attempt as u32,
            })
            .collect())
    }

    async fn ack(&self, _queue: &str, receipt: &str) -> Result<(), CacheError> {
        let result = sqlx::query(
            r#"
            DELETE FROM _reactor_cache.queue
            WHERE receipt = $1
            "#,
        )
        .bind(receipt)
        .execute(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        if result.rows_affected() == 0 {
            return Err(CacheError::InvalidReceipt(receipt.to_string()));
        }

        Ok(())
    }

    async fn nack(
        &self,
        _queue: &str,
        receipt: &str,
        delay: Option<Duration>,
    ) -> Result<(), CacheError> {
        let result = match delay {
            Some(d) => {
                let visible_at =
                    Utc::now() + chrono::Duration::from_std(d).unwrap_or_default();
                sqlx::query(
                    r#"
                    UPDATE _reactor_cache.queue
                    SET receipt = NULL,
                        visible_at = $2
                    WHERE receipt = $1
                    "#,
                )
                .bind(receipt)
                .bind(visible_at)
                .execute(&self.pool)
                .await
            }
            None => {
                // Use SQL now() for immediate visibility to avoid clock skew
                sqlx::query(
                    r#"
                    UPDATE _reactor_cache.queue
                    SET receipt = NULL,
                        visible_at = now()
                    WHERE receipt = $1
                    "#,
                )
                .bind(receipt)
                .execute(&self.pool)
                .await
            }
        }
        .map_err(CacheError::Database)?;

        if result.rows_affected() == 0 {
            return Err(CacheError::InvalidReceipt(receipt.to_string()));
        }

        Ok(())
    }

    async fn queue_len(&self, queue: &str) -> Result<u64, CacheError> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM _reactor_cache.queue
            WHERE queue_name = $1
            "#,
        )
        .bind(queue)
        .fetch_one(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        Ok(row.0 as u64)
    }
}

#[async_trait]
impl KvOperations for PostgresBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let row: Option<KvRow> = sqlx::query_as(
            r#"
            SELECT key, value, expires_at
            FROM _reactor_cache.kv
            WHERE key = $1
              AND (expires_at IS NULL OR expires_at > now())
            "#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        Ok(row.map(|r| r.value))
    }

    async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<(), CacheError> {
        let expires_at = ttl.map(|d| Utc::now() + chrono::Duration::from_std(d).unwrap_or_default());

        sqlx::query(
            r#"
            INSERT INTO _reactor_cache.kv (key, value, expires_at, updated_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (key) DO UPDATE
            SET value = EXCLUDED.value,
                expires_at = EXCLUDED.expires_at,
                updated_at = now()
            "#,
        )
        .bind(key)
        .bind(value)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        Ok(())
    }

    async fn del(&self, key: &str) -> Result<bool, CacheError> {
        let result = sqlx::query(
            r#"
            DELETE FROM _reactor_cache.kv
            WHERE key = $1
            "#,
        )
        .bind(key)
        .execute(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool, CacheError> {
        let expires_at = Utc::now() + chrono::Duration::from_std(ttl).unwrap_or_default();

        let result = sqlx::query(
            r#"
            UPDATE _reactor_cache.kv
            SET expires_at = $2, updated_at = now()
            WHERE key = $1
            "#,
        )
        .bind(key)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        let row: (bool,) = sqlx::query_as(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM _reactor_cache.kv
                WHERE key = $1
                  AND (expires_at IS NULL OR expires_at > now())
            )
            "#,
        )
        .bind(key)
        .fetch_one(&self.pool)
        .await
        .map_err(CacheError::Database)?;

        Ok(row.0)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct QueueRow {
    id: Uuid,
    receipt: Option<String>,
    data: Vec<u8>,
    enqueued_at: DateTime<Utc>,
    attempt: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct KvRow {
    #[allow(dead_code)]
    key: String,
    value: Vec<u8>,
    #[allow(dead_code)]
    expires_at: Option<DateTime<Utc>>,
}

/// Migration SQL for the cache schema.
const MIGRATION_SQL: &str = r#"
CREATE SCHEMA IF NOT EXISTS _reactor_cache;

-- Queue table (SKIP LOCKED pattern)
CREATE TABLE IF NOT EXISTS _reactor_cache.queue (
    id              uuid PRIMARY KEY,
    queue_name      text NOT NULL,
    data            bytea NOT NULL,
    visible_at      timestamptz NOT NULL DEFAULT now(),
    attempt         integer NOT NULL DEFAULT 0,
    created_at      timestamptz NOT NULL DEFAULT now(),
    receipt         text UNIQUE
);

CREATE INDEX IF NOT EXISTS idx_queue_dequeue 
    ON _reactor_cache.queue (queue_name, visible_at) 
    WHERE receipt IS NULL;

-- KV table
CREATE TABLE IF NOT EXISTS _reactor_cache.kv (
    key             text PRIMARY KEY,
    value           bytea NOT NULL,
    expires_at      timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_kv_expires 
    ON _reactor_cache.kv (expires_at) 
    WHERE expires_at IS NOT NULL;
"#;
