//! Connector sink - writes records to another connector instance (outbound sync).
//!
//! This sink wraps a ConnectorRuntime to deliver records to another instance,
//! enabling reverse sync (e.g., Reactor → Salesforce).

use super::{DestinationSink, SinkWriteOutcome, StreamStats};
use crate::error::ConnectError;
use crate::protocol::{AirbyteRecordMessage, AirbyteStateMessage, ConnectorMessage, SyncLimits};
use crate::runtime::ConnectorRuntime;
use async_trait::async_trait;
use futures::stream;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for the connector sink.
#[derive(Debug, Clone)]
pub struct ConnectorSinkConfig {
    /// Target connector type ID.
    pub connector_type: String,
    /// Target stream name.
    pub stream: String,
    /// Instance configuration (credentials, etc.).
    pub config: Value,
    /// Batch size for writes.
    pub batch_size: usize,
}

impl Default for ConnectorSinkConfig {
    fn default() -> Self {
        Self {
            connector_type: String::new(),
            stream: String::new(),
            config: Value::Null,
            batch_size: 100,
        }
    }
}

/// Sink that writes records to another connector instance.
pub struct ConnectorSink<R: ConnectorRuntime> {
    runtime: Arc<R>,
    config: ConnectorSinkConfig,
    stats: RwLock<HashMap<String, StreamStats>>,
}

impl<R: ConnectorRuntime> ConnectorSink<R> {
    /// Create a new connector sink.
    pub fn new(runtime: Arc<R>, config: ConnectorSinkConfig) -> Self {
        Self {
            runtime,
            config,
            stats: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl<R: ConnectorRuntime + 'static> DestinationSink for ConnectorSink<R> {
    async fn init(
        &self,
        _org_id: &Uuid,
        _connection_id: &Uuid,
        _streams: &[String],
    ) -> Result<(), ConnectError> {
        // Verify the runtime supports writing to this connector type
        let descriptor = self.runtime.descriptor(&self.config.connector_type).await?;
        
        // Check if the stream supports outbound
        let stream = descriptor
            .streams
            .iter()
            .find(|s| s.name == self.config.stream);
        
        if let Some(stream) = stream {
            if !stream.supports_outbound {
                return Err(ConnectError::InvalidInput(format!(
                    "Stream '{}' does not support outbound writes",
                    self.config.stream
                )));
            }
        }
        
        self.stats.write().await.clear();
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

        let mut total_bytes = 0u64;

        // Convert records to a message stream
        let messages: Vec<Result<ConnectorMessage, ConnectError>> = records
            .iter()
            .map(|r| {
                let bytes = serde_json::to_vec(&r.data).unwrap_or_default().len() as u64;
                total_bytes += bytes;
                Ok(ConnectorMessage::Record(r.clone()))
            })
            .collect();

        let message_stream = Box::pin(stream::iter(messages));

        // Call the runtime's write method
        let limits = SyncLimits::default();
        let outcome = self
            .runtime
            .write(
                &self.config.connector_type,
                &self.config.config,
                stream,
                message_stream,
                &limits,
            )
            .await?;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            let stat = stats.entry(stream.to_string()).or_default();
            stat.records += outcome.records_written;
            stat.bytes += total_bytes;
        }

        let mut stream_stats = HashMap::new();
        stream_stats.insert(
            stream.to_string(),
            StreamStats {
                records: outcome.records_written,
                bytes: total_bytes,
            },
        );

        Ok(SinkWriteOutcome {
            records_written: outcome.records_written,
            bytes_written: total_bytes,
            stream_stats,
        })
    }

    async fn checkpoint(&self, _state: &AirbyteStateMessage) -> Result<(), ConnectError> {
        // Connector sink doesn't maintain state - it just writes through
        Ok(())
    }

    async fn finalize(&self) -> Result<SinkWriteOutcome, ConnectError> {
        let stats = self.stats.read().await;
        let mut total_records = 0u64;
        let mut total_bytes = 0u64;

        for stat in stats.values() {
            total_records += stat.records;
            total_bytes += stat.bytes;
        }

        Ok(SinkWriteOutcome {
            records_written: total_records,
            bytes_written: total_bytes,
            stream_stats: stats.clone(),
        })
    }

    async fn abort(&self, _reason: &str) -> Result<(), ConnectError> {
        // No cleanup needed for connector sink
        self.stats.write().await.clear();
        Ok(())
    }
}
