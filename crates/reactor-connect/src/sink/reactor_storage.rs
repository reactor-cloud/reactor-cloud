//! ReactorStorageSink: Uploads records to reactor-storage via HTTP.
//!
//! This sink stores records as files in reactor-storage, suitable for
//! large datasets or when file-based delivery is preferred.

use crate::error::ConnectError;
use crate::protocol::{AirbyteRecordMessage, AirbyteStateMessage};
use crate::sink::{DestinationSink, SinkWriteOutcome, StreamStats};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for ReactorStorageSink.
#[derive(Debug, Clone)]
pub struct ReactorStorageSinkConfig {
    /// Base URL for reactor-storage.
    pub base_url: String,
    /// Auth token for internal requests.
    pub auth_token: Option<String>,
    /// Bucket name.
    pub bucket: String,
    /// Path prefix within the bucket.
    pub path_prefix: Option<String>,
    /// File format (jsonl, parquet, csv).
    pub format: StorageFormat,
    /// Max file size in bytes before rotating.
    pub max_file_size: u64,
}

/// Storage file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageFormat {
    /// JSON Lines format (one JSON object per line).
    JsonLines,
    /// Parquet format.
    Parquet,
    /// CSV format.
    Csv,
}

impl Default for ReactorStorageSinkConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".to_string(),
            auth_token: None,
            bucket: "sync-data".to_string(),
            path_prefix: None,
            format: StorageFormat::JsonLines,
            max_file_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// Buffered records for a stream.
struct StreamBuffer {
    records: Vec<AirbyteRecordMessage>,
    bytes: u64,
    file_count: u32,
}

/// Sink that uploads records to reactor-storage.
pub struct ReactorStorageSink {
    config: ReactorStorageSinkConfig,
    http: reqwest::Client,
    org_id: RwLock<Option<Uuid>>,
    connection_id: RwLock<Option<Uuid>>,
    buffers: Arc<RwLock<HashMap<String, StreamBuffer>>>,
    stats: Arc<RwLock<HashMap<String, StreamStats>>>,
    total_records: AtomicU64,
    total_bytes: AtomicU64,
}

impl ReactorStorageSink {
    /// Create a new ReactorStorageSink.
    pub fn new(config: ReactorStorageSinkConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
            org_id: RwLock::new(None),
            connection_id: RwLock::new(None),
            buffers: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HashMap::new())),
            total_records: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
        }
    }

    /// Build the storage path for a stream.
    fn storage_path(&self, connection_id: &Uuid, stream: &str, file_num: u32) -> String {
        let date = Utc::now().format("%Y/%m/%d");
        let timestamp = Utc::now().format("%H%M%S");
        let extension = match self.config.format {
            StorageFormat::JsonLines => "jsonl",
            StorageFormat::Parquet => "parquet",
            StorageFormat::Csv => "csv",
        };

        match &self.config.path_prefix {
            Some(prefix) => format!(
                "{}/{}/{}/{}/{}_{:04}.{}",
                prefix, connection_id, stream, date, timestamp, file_num, extension
            ),
            None => format!(
                "{}/{}/{}/{}_{:04}.{}",
                connection_id, stream, date, timestamp, file_num, extension
            ),
        }
    }

    /// Flush a stream buffer to storage.
    async fn flush_buffer(&self, stream: &str, buffer: &mut StreamBuffer) -> Result<u64, ConnectError> {
        if buffer.records.is_empty() {
            return Ok(0);
        }

        let org_id = self.org_id.read().await.ok_or_else(|| {
            ConnectError::Internal("sink not initialized".to_string())
        })?;

        let connection_id = self.connection_id.read().await.ok_or_else(|| {
            ConnectError::Internal("sink not initialized".to_string())
        })?;

        let path = self.storage_path(&connection_id, stream, buffer.file_count);
        let url = format!(
            "{}/storage/v1/orgs/{}/buckets/{}/objects/{}",
            self.config.base_url, org_id, self.config.bucket, path
        );

        // Convert records to the appropriate format
        let (body, content_type) = match self.config.format {
            StorageFormat::JsonLines => {
                let mut lines = Vec::new();
                for record in &buffer.records {
                    let line = serde_json::to_string(&record.data).map_err(|e| {
                        ConnectError::Internal(format!("failed to serialize record: {}", e))
                    })?;
                    lines.push(line);
                }
                (lines.join("\n"), "application/x-jsonlines")
            }
            StorageFormat::Csv => {
                // Simple CSV implementation - in production would use csv crate
                let mut csv = String::new();
                for record in &buffer.records {
                    let line = serde_json::to_string(&record.data).map_err(|e| {
                        ConnectError::Internal(format!("failed to serialize record: {}", e))
                    })?;
                    csv.push_str(&line);
                    csv.push('\n');
                }
                (csv, "text/csv")
            }
            StorageFormat::Parquet => {
                // Parquet would require arrow/parquet crates
                // For now, fall back to JSONL
                let mut lines = Vec::new();
                for record in &buffer.records {
                    let line = serde_json::to_string(&record.data).map_err(|e| {
                        ConnectError::Internal(format!("failed to serialize record: {}", e))
                    })?;
                    lines.push(line);
                }
                (lines.join("\n"), "application/x-jsonlines")
            }
        };

        let bytes_written = body.len() as u64;

        let mut request = self.http.put(&url).body(body).header("Content-Type", content_type);

        if let Some(token) = &self.config.auth_token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await.map_err(|e| {
            ConnectError::Internal(format!("failed to upload to reactor-storage: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ConnectError::Internal(format!(
                "reactor-storage returned {}: {}",
                status, body
            )));
        }

        let records_written = buffer.records.len() as u64;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            let entry = stats.entry(stream.to_string()).or_default();
            entry.records += records_written;
            entry.bytes += bytes_written;
        }

        self.total_records.fetch_add(records_written, Ordering::SeqCst);
        self.total_bytes.fetch_add(bytes_written, Ordering::SeqCst);

        // Clear buffer and increment file count
        buffer.records.clear();
        buffer.bytes = 0;
        buffer.file_count += 1;

        Ok(bytes_written)
    }
}

#[async_trait]
impl DestinationSink for ReactorStorageSink {
    async fn init(
        &self,
        org_id: &Uuid,
        connection_id: &Uuid,
        _streams: &[String],
    ) -> Result<(), ConnectError> {
        *self.org_id.write().await = Some(*org_id);
        *self.connection_id.write().await = Some(*connection_id);

        // Reset state
        self.buffers.write().await.clear();
        self.stats.write().await.clear();
        self.total_records.store(0, Ordering::SeqCst);
        self.total_bytes.store(0, Ordering::SeqCst);

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

        let mut buffers = self.buffers.write().await;
        let buffer = buffers.entry(stream.to_string()).or_insert_with(|| StreamBuffer {
            records: Vec::new(),
            bytes: 0,
            file_count: 0,
        });

        let mut bytes_written = 0u64;

        for record in records {
            let record_bytes = serde_json::to_string(&record.data)
                .map(|s| s.len() as u64)
                .unwrap_or(0);

            buffer.records.push(record.clone());
            buffer.bytes += record_bytes;

            // Flush if buffer exceeds max file size
            if buffer.bytes >= self.config.max_file_size {
                bytes_written += self.flush_buffer(stream, buffer).await?;
            }
        }

        let mut stream_stats = HashMap::new();
        stream_stats.insert(
            stream.to_string(),
            StreamStats {
                records: records.len() as u64,
                bytes: bytes_written,
            },
        );

        Ok(SinkWriteOutcome {
            records_written: records.len() as u64,
            bytes_written,
            stream_stats,
        })
    }

    async fn checkpoint(&self, _state: &AirbyteStateMessage) -> Result<(), ConnectError> {
        // Flush all buffers on checkpoint
        let mut buffers = self.buffers.write().await;
        for (stream, buffer) in buffers.iter_mut() {
            if !buffer.records.is_empty() {
                self.flush_buffer(stream, buffer).await?;
            }
        }
        Ok(())
    }

    async fn finalize(&self) -> Result<SinkWriteOutcome, ConnectError> {
        // Flush any remaining buffers
        let mut buffers = self.buffers.write().await;
        for (stream, buffer) in buffers.iter_mut() {
            if !buffer.records.is_empty() {
                self.flush_buffer(stream, buffer).await?;
            }
        }

        let stats = self.stats.read().await.clone();

        Ok(SinkWriteOutcome {
            records_written: self.total_records.load(Ordering::SeqCst),
            bytes_written: self.total_bytes.load(Ordering::SeqCst),
            stream_stats: stats,
        })
    }

    async fn abort(&self, _reason: &str) -> Result<(), ConnectError> {
        // Clear buffers without flushing
        self.buffers.write().await.clear();
        Ok(())
    }
}
