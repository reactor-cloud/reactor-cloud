//! EphemeralSink: Stores records in an ephemeral _sandbox_* schema.
//!
//! This sink creates a temporary database schema for testing syncs
//! in sandbox mode. Records are stored in a schema that can be
//! inspected and then dropped after the sandbox expires.

use crate::error::ConnectError;
use crate::protocol::{AirbyteRecordMessage, AirbyteStateMessage};
use crate::sink::{DestinationSink, SinkWriteOutcome, StreamStats};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for EphemeralSink.
#[derive(Debug, Clone)]
pub struct EphemeralSinkConfig {
    /// TTL for the ephemeral schema.
    pub ttl: Duration,
    /// Maximum records to store per stream.
    pub max_records_per_stream: u64,
    /// Maximum total records.
    pub max_total_records: u64,
}

impl Default for EphemeralSinkConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::hours(24),
            max_records_per_stream: 10_000,
            max_total_records: 100_000,
        }
    }
}

/// Metadata about an ephemeral schema.
#[derive(Debug, Clone)]
pub struct EphemeralSchemaInfo {
    /// Schema name.
    pub schema_name: String,
    /// Connection ID that owns this schema.
    pub connection_id: Uuid,
    /// Organization ID.
    pub org_id: Uuid,
    /// When the schema was created.
    pub created_at: DateTime<Utc>,
    /// When the schema expires.
    pub expires_at: DateTime<Utc>,
    /// Whether the schema has been promoted to permanent.
    pub promoted: bool,
}

/// Sink that stores records in an ephemeral Postgres schema.
pub struct EphemeralSink {
    pool: PgPool,
    config: EphemeralSinkConfig,
    org_id: RwLock<Option<Uuid>>,
    connection_id: RwLock<Option<Uuid>>,
    schema_name: RwLock<Option<String>>,
    stats: Arc<RwLock<HashMap<String, StreamStats>>>,
    total_records: AtomicU64,
    total_bytes: AtomicU64,
}

impl EphemeralSink {
    /// Create a new EphemeralSink.
    pub fn new(pool: PgPool, config: EphemeralSinkConfig) -> Self {
        Self {
            pool,
            config,
            org_id: RwLock::new(None),
            connection_id: RwLock::new(None),
            schema_name: RwLock::new(None),
            stats: Arc::new(RwLock::new(HashMap::new())),
            total_records: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
        }
    }

    /// Generate a unique schema name for a sandbox.
    fn generate_schema_name(connection_id: &Uuid) -> String {
        let short_id = &connection_id.to_string()[..8];
        let timestamp = Utc::now().timestamp();
        format!("_sandbox_{}_{}", short_id, timestamp)
    }

    /// Sanitize a table name (stream name).
    fn sanitize_table_name(stream: &str) -> String {
        stream
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect()
    }

    /// Get the schema name.
    pub async fn schema_name(&self) -> Option<String> {
        self.schema_name.read().await.clone()
    }

    /// Get schema info.
    pub async fn schema_info(&self) -> Option<EphemeralSchemaInfo> {
        let schema_name = self.schema_name.read().await.clone()?;
        let org_id = (*self.org_id.read().await)?;
        let connection_id = (*self.connection_id.read().await)?;

        Some(EphemeralSchemaInfo {
            schema_name,
            connection_id,
            org_id,
            created_at: Utc::now(), // Would fetch from DB in real impl
            expires_at: Utc::now() + self.config.ttl,
            promoted: false,
        })
    }

    /// Create a table for a stream if it doesn't exist.
    async fn ensure_table(&self, stream: &str) -> Result<(), ConnectError> {
        let schema = self.schema_name.read().await.clone().ok_or_else(|| {
            ConnectError::Internal("sink not initialized".to_string())
        })?;

        let table = Self::sanitize_table_name(stream);

        // Create table with JSONB data column
        let query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS "{}"."{}" (
                id SERIAL PRIMARY KEY,
                data JSONB NOT NULL,
                emitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            schema, table
        );

        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| ConnectError::Internal(format!("failed to create table: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl DestinationSink for EphemeralSink {
    async fn init(
        &self,
        org_id: &Uuid,
        connection_id: &Uuid,
        streams: &[String],
    ) -> Result<(), ConnectError> {
        *self.org_id.write().await = Some(*org_id);
        *self.connection_id.write().await = Some(*connection_id);

        // Generate schema name
        let schema_name = Self::generate_schema_name(connection_id);

        // Create schema
        let query = format!(r#"CREATE SCHEMA IF NOT EXISTS "{}""#, schema_name);
        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| ConnectError::Internal(format!("failed to create schema: {}", e)))?;

        *self.schema_name.write().await = Some(schema_name.clone());

        // Create tables for each stream
        for stream in streams {
            self.ensure_table(stream).await?;
        }

        // Reset stats
        self.stats.write().await.clear();
        self.total_records.store(0, Ordering::SeqCst);
        self.total_bytes.store(0, Ordering::SeqCst);

        // Record schema in metadata table
        let expires_at = Utc::now() + self.config.ttl;
        let schema_id = Uuid::new_v4();
        // Generate a placeholder promote token hash (real implementation would use HMAC)
        let promote_token_hash = vec![0u8; 32]; // placeholder

        sqlx::query(
            r#"
            INSERT INTO _reactor_connect.sandbox_schemas (
                id, connection_id, schema_name, promote_token_hash, diff_json, created_at, expires_at
            ) VALUES ($1, $2, $3, $4, '{}'::jsonb, NOW(), $5)
            ON CONFLICT (schema_name) DO UPDATE SET
                expires_at = $5
            "#,
        )
        .bind(schema_id)
        .bind(connection_id)
        .bind(&schema_name)
        .bind(&promote_token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectError::Internal(format!("failed to record schema metadata: {}", e)))?;

        Ok(())
    }

    async fn write_batch(
        &self,
        stream: &str,
        records: &[AirbyteRecordMessage],
    ) -> Result<SinkWriteOutcome, ConnectError> {
        if records.is_empty() {
            return Ok(SinkWriteOutcome {
                records_written: 0,
                bytes_written: 0,
                stream_stats: HashMap::new(),
            });
        }

        // Check limits
        let current_total = self.total_records.load(Ordering::SeqCst);
        if current_total >= self.config.max_total_records {
            return Err(ConnectError::Internal(format!(
                "sandbox limit exceeded: {} records",
                self.config.max_total_records
            )));
        }

        // Ensure table exists
        self.ensure_table(stream).await?;

        let schema = self.schema_name.read().await.clone().ok_or_else(|| {
            ConnectError::Internal("sink not initialized".to_string())
        })?;

        let table = Self::sanitize_table_name(stream);
        let mut bytes_written = 0u64;

        // Insert records in batches
        for record in records {
            let data_json = serde_json::to_string(&record.data).map_err(|e| {
                ConnectError::Internal(format!("failed to serialize record: {}", e))
            })?;

            bytes_written += data_json.len() as u64;

            let emitted_at = DateTime::from_timestamp_millis(record.emitted_at)
                .unwrap_or_else(Utc::now);

            let query = format!(
                r#"INSERT INTO "{}"."{}" (data, emitted_at) VALUES ($1::jsonb, $2)"#,
                schema, table
            );

            sqlx::query(&query)
                .bind(&data_json)
                .bind(emitted_at)
                .execute(&self.pool)
                .await
                .map_err(|e| ConnectError::Internal(format!("failed to insert record: {}", e)))?;
        }

        let records_written = records.len() as u64;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            let entry = stats.entry(stream.to_string()).or_default();
            entry.records += records_written;
            entry.bytes += bytes_written;
        }

        self.total_records.fetch_add(records_written, Ordering::SeqCst);
        self.total_bytes.fetch_add(bytes_written, Ordering::SeqCst);

        let mut stream_stats = HashMap::new();
        stream_stats.insert(
            stream.to_string(),
            StreamStats {
                records: records_written,
                bytes: bytes_written,
            },
        );

        Ok(SinkWriteOutcome {
            records_written,
            bytes_written,
            stream_stats,
        })
    }

    async fn checkpoint(&self, _state: &AirbyteStateMessage) -> Result<(), ConnectError> {
        // Postgres transactions handle durability
        Ok(())
    }

    async fn finalize(&self) -> Result<SinkWriteOutcome, ConnectError> {
        let stats = self.stats.read().await.clone();

        Ok(SinkWriteOutcome {
            records_written: self.total_records.load(Ordering::SeqCst),
            bytes_written: self.total_bytes.load(Ordering::SeqCst),
            stream_stats: stats,
        })
    }

    async fn abort(&self, _reason: &str) -> Result<(), ConnectError> {
        // Drop the ephemeral schema
        if let Some(schema) = self.schema_name.read().await.clone() {
            let query = format!(r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#, schema);
            let _ = sqlx::query(&query).execute(&self.pool).await;

            // Remove from metadata table
            let _ = sqlx::query("DELETE FROM _reactor_connect.sandbox_schemas WHERE schema_name = $1")
                .bind(&schema)
                .execute(&self.pool)
                .await;
        }

        Ok(())
    }
}

/// Cleanup worker for expired ephemeral schemas.
pub async fn cleanup_expired_schemas(pool: &PgPool) -> Result<u64, ConnectError> {
    // Find expired schemas
    let expired: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT schema_name
        FROM _reactor_connect.sandbox_schemas
        WHERE expires_at < NOW() AND NOT promoted
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ConnectError::Internal(format!("failed to query expired schemas: {}", e)))?;

    let mut dropped = 0u64;

    for (schema_name,) in expired {
        // Drop schema
        let query = format!(r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#, schema_name);
        if sqlx::query(&query).execute(pool).await.is_ok() {
            // Remove from metadata
            let _ = sqlx::query("DELETE FROM _reactor_connect.sandbox_schemas WHERE schema_name = $1")
                .bind(&schema_name)
                .execute(pool)
                .await;
            dropped += 1;
        }
    }

    Ok(dropped)
}
