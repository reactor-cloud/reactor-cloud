//! ReactorDataSink: Posts records to reactor-data via HTTP.
//!
//! This sink sends records to the reactor-data capability's ingestion
//! endpoint for storage in the Reactor data warehouse.

use crate::error::ConnectError;
use crate::protocol::{AirbyteRecordMessage, AirbyteStateMessage};
use crate::sink::{DestinationSink, SinkWriteOutcome, StreamStats};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for ReactorDataSink.
#[derive(Debug, Clone)]
pub struct ReactorDataSinkConfig {
    /// Base URL for reactor-data (e.g., "http://localhost:3000" or internal service URL).
    pub base_url: String,
    /// Auth token for internal requests.
    pub auth_token: Option<String>,
    /// Batch size for writes.
    pub batch_size: usize,
    /// Dataset name prefix.
    pub dataset_prefix: Option<String>,
}

impl Default for ReactorDataSinkConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".to_string(),
            auth_token: None,
            batch_size: 1000,
            dataset_prefix: None,
        }
    }
}

/// Sink that posts records to reactor-data.
pub struct ReactorDataSink {
    config: ReactorDataSinkConfig,
    http: reqwest::Client,
    org_id: RwLock<Option<Uuid>>,
    connection_id: RwLock<Option<Uuid>>,
    stats: Arc<RwLock<HashMap<String, StreamStats>>>,
    total_records: AtomicU64,
    total_bytes: AtomicU64,
}

impl ReactorDataSink {
    /// Create a new ReactorDataSink.
    pub fn new(config: ReactorDataSinkConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
            org_id: RwLock::new(None),
            connection_id: RwLock::new(None),
            stats: Arc::new(RwLock::new(HashMap::new())),
            total_records: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
        }
    }

    /// Build the dataset name for a stream.
    fn dataset_name(&self, stream: &str) -> String {
        match &self.config.dataset_prefix {
            Some(prefix) => format!("{}_{}", prefix, stream),
            None => stream.to_string(),
        }
    }
}

#[async_trait]
impl DestinationSink for ReactorDataSink {
    async fn init(
        &self,
        org_id: &Uuid,
        connection_id: &Uuid,
        _streams: &[String],
    ) -> Result<(), ConnectError> {
        *self.org_id.write().await = Some(*org_id);
        *self.connection_id.write().await = Some(*connection_id);

        // Reset stats
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

        let org_id = self.org_id.read().await.ok_or_else(|| {
            ConnectError::Internal("sink not initialized".to_string())
        })?;

        let dataset = self.dataset_name(stream);
        let url = format!("{}/data/v1/orgs/{}/datasets/{}/records", self.config.base_url, org_id, dataset);

        // Convert records to JSON array
        let payload: Vec<serde_json::Value> = records.iter().map(|r| r.data.clone()).collect();
        let payload_json = serde_json::to_string(&payload).map_err(|e| {
            ConnectError::Internal(format!("failed to serialize records: {}", e))
        })?;

        let bytes_written = payload_json.len() as u64;

        let mut request = self.http.post(&url).json(&payload);

        if let Some(token) = &self.config.auth_token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await.map_err(|e| {
            ConnectError::Internal(format!("failed to send records to reactor-data: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ConnectError::Internal(format!(
                "reactor-data returned {}: {}",
                status, body
            )));
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
        // reactor-data handles durability on its end, nothing special to do
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
        // reactor-data is append-only; we can't roll back
        // In a real implementation, we might mark records as invalid
        // or use a staging table that gets dropped
        Ok(())
    }
}
