//! Stream sync executor.
//!
//! Orchestrates the sync loop: read → buffer → sink → state.

use crate::error::ConnectError;
use crate::protocol::{AirbyteLogLevel, ConfiguredCatalog, ConnectorMessage, SyncLimits};
use crate::runtime::ConnectorRuntime;
use crate::sink::{DestinationSink, SinkWriteOutcome};
use crate::store::{ConnectStore, Connection};
use crate::sync::{RecordBuffer, StateManager};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn, instrument};

/// Sync execution options.
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// Maximum records per batch before flushing.
    pub batch_size: u64,
    /// Maximum bytes per batch before flushing.
    pub batch_bytes: u64,
    /// Maximum duration for the sync run.
    pub max_duration: Option<Duration>,
    /// Maximum total records to sync.
    pub max_records: Option<u64>,
    /// Whether this is a sandbox run.
    pub sandbox: bool,
    /// Checkpoint every N records.
    pub checkpoint_interval: u64,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            batch_bytes: 10 * 1024 * 1024, // 10MB
            max_duration: None,
            max_records: None,
            sandbox: false,
            checkpoint_interval: 10_000,
        }
    }
}

/// Outcome of a sync run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOutcome {
    /// Whether the sync succeeded.
    pub success: bool,
    /// Total records read.
    pub records_read: u64,
    /// Total records written.
    pub records_written: u64,
    /// Bytes written.
    pub bytes_written: u64,
    /// Per-stream stats.
    pub stream_stats: std::collections::HashMap<String, StreamSyncStats>,
    /// Error if failed.
    pub error: Option<SyncError>,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Per-stream sync statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamSyncStats {
    /// Records read from this stream.
    pub records_read: u64,
    /// Records written for this stream.
    pub records_written: u64,
    /// Bytes written for this stream.
    pub bytes_written: u64,
}

/// Sync error details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncError {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
    /// Suggested fix.
    pub suggested_fix: Option<String>,
}

/// The sync executor.
pub struct SyncExecutor<S: ConnectStore> {
    store: S,
    runtime: Arc<dyn ConnectorRuntime>,
}

impl<S: ConnectStore> SyncExecutor<S> {
    /// Create a new sync executor.
    pub fn new(store: S, runtime: Arc<dyn ConnectorRuntime>) -> Self {
        Self { store, runtime }
    }

    /// Execute a sync run.
    #[instrument(skip(self, sink, config, catalog), fields(connection_id = %connection.id))]
    pub async fn execute(
        &self,
        connection: &Connection,
        config: &serde_json::Value,
        catalog: &ConfiguredCatalog,
        sink: &dyn DestinationSink,
        options: &SyncOptions,
    ) -> Result<SyncOutcome, ConnectError> {
        let start = Instant::now();
        let connection_id = connection.id;

        info!(
            connection_id = %connection_id,
            streams = catalog.streams.len(),
            sandbox = options.sandbox,
            "starting sync"
        );

        // Initialize state manager
        let stream_names: Vec<String> = catalog.streams.iter().map(|s| s.stream.clone()).collect();
        let mut state_manager = StateManager::new(self.store.clone(), connection_id);
        state_manager.load(&stream_names).await?;

        // Initialize sink
        sink.init(&connection.org_id, &connection_id, &stream_names).await?;

        // Get the connector type
        let source_instance_id = connection.source_instance_id
            .ok_or_else(|| ConnectError::Internal("connection has no source instance".to_string()))?;
        let source_instance = self.store.get_instance_by_id(&source_instance_id)
            .await?
            .ok_or_else(|| ConnectError::Internal("source instance not found".to_string()))?;

        // Build sync limits
        let limits = SyncLimits {
            max_rows: options.max_records,
            max_duration_seconds: options.max_duration.map(|d| d.as_secs()),
            sandbox: options.sandbox,
        };

        // Start the read stream
        let state_bundle = state_manager.as_bundle();
        let mut message_stream = self.runtime
            .read(&source_instance.type_id, config, catalog, state_bundle.as_ref(), &limits)
            .await?;

        // Process messages
        let mut buffer = RecordBuffer::new();
        let mut total_records_read = 0u64;
        let mut total_records_written = 0u64;
        let mut total_bytes_written = 0u64;
        let mut stream_stats: std::collections::HashMap<String, StreamSyncStats> = std::collections::HashMap::new();
        let mut records_since_checkpoint = 0u64;

        let result: Result<(), ConnectError> = async {
            while let Some(msg_result) = message_stream.next().await {
                // Check duration limit
                if let Some(max_dur) = options.max_duration {
                    if start.elapsed() > max_dur {
                        warn!("sync duration limit exceeded");
                        break;
                    }
                }

                let msg = msg_result?;

                match msg {
                    ConnectorMessage::Record(record) => {
                        total_records_read += 1;
                        records_since_checkpoint += 1;

                        // Update per-stream stats
                        stream_stats
                            .entry(record.stream.clone())
                            .or_default()
                            .records_read += 1;

                        buffer.push(record);

                        // Flush if batch is full
                        if buffer.len() >= options.batch_size || buffer.total_bytes >= options.batch_bytes {
                            let outcome = flush_and_track(&mut buffer, sink, &mut stream_stats).await?;
                            total_records_written += outcome.records_written;
                            total_bytes_written += outcome.bytes_written;
                        }

                        // Checkpoint periodically
                        if records_since_checkpoint >= options.checkpoint_interval {
                            flush_and_track(&mut buffer, sink, &mut stream_stats).await?;
                            state_manager.persist().await?;
                            records_since_checkpoint = 0;
                            debug!(records = total_records_read, "checkpoint persisted");
                        }

                        // Check record limit
                        if let Some(max) = options.max_records {
                            if total_records_read >= max {
                                info!("record limit reached");
                                break;
                            }
                        }
                    }
                    ConnectorMessage::State(state) => {
                        // Record state checkpoint
                        state_manager.checkpoint(&state);
                        sink.checkpoint(&state).await?;
                    }
                    ConnectorMessage::Log(log) => {
                        match log.level {
                            AirbyteLogLevel::Error | AirbyteLogLevel::Fatal => {
                                error!(message = %log.message, "connector log")
                            }
                            AirbyteLogLevel::Warn => warn!(message = %log.message, "connector log"),
                            AirbyteLogLevel::Info => info!(message = %log.message, "connector log"),
                            AirbyteLogLevel::Debug | AirbyteLogLevel::Trace => {
                                debug!(message = %log.message, "connector log")
                            }
                        }
                    }
                    ConnectorMessage::Trace(trace) => {
                        if let Some(error) = trace.error {
                            error!(
                                code = ?error.failure_type,
                                message = %error.message,
                                "connector trace error"
                            );
                            return Err(ConnectError::ActionFailed {
                                code: format!("{:?}", error.failure_type),
                                cause: error.message,
                                suggested_fix: error.stack_trace,
                            });
                        }
                    }
                    _ => {
                        // Ignore other message types
                    }
                }
            }

            // Final flush
            if !buffer.is_empty() {
                let outcome = flush_and_track(&mut buffer, sink, &mut stream_stats).await?;
                total_records_written += outcome.records_written;
                total_bytes_written += outcome.bytes_written;
            }

            // Final state persist
            state_manager.persist().await?;

            // Finalize sink
            sink.finalize().await?;

            Ok(())
        }.await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(()) => {
                info!(
                    records_read = total_records_read,
                    records_written = total_records_written,
                    duration_ms = duration_ms,
                    "sync completed successfully"
                );

                Ok(SyncOutcome {
                    success: true,
                    records_read: total_records_read,
                    records_written: total_records_written,
                    bytes_written: total_bytes_written,
                    stream_stats,
                    error: None,
                    duration_ms,
                })
            }
            Err(e) => {
                error!(error = %e, "sync failed");

                // Abort sink
                let _ = sink.abort(&e.to_string()).await;

                let sync_error = match &e {
                    ConnectError::ActionFailed { code, cause, suggested_fix } => SyncError {
                        code: code.clone(),
                        message: cause.clone(),
                        suggested_fix: suggested_fix.clone(),
                    },
                    other => SyncError {
                        code: "SYNC_ERROR".to_string(),
                        message: other.to_string(),
                        suggested_fix: None,
                    },
                };

                Ok(SyncOutcome {
                    success: false,
                    records_read: total_records_read,
                    records_written: total_records_written,
                    bytes_written: total_bytes_written,
                    stream_stats,
                    error: Some(sync_error),
                    duration_ms,
                })
            }
        }
    }
}

/// Flush buffer and update stream stats.
async fn flush_and_track(
    buffer: &mut RecordBuffer,
    sink: &dyn DestinationSink,
    stream_stats: &mut std::collections::HashMap<String, StreamSyncStats>,
) -> Result<SinkWriteOutcome, ConnectError> {
    let mut total_outcome = SinkWriteOutcome {
        records_written: 0,
        bytes_written: 0,
        stream_stats: std::collections::HashMap::new(),
    };

    for (stream, records) in buffer.streams.drain() {
        if !records.is_empty() {
            let outcome = sink.write_batch(&stream, &records).await?;
            
            // Update stream stats
            let stats = stream_stats.entry(stream.clone()).or_default();
            stats.records_written += outcome.records_written;
            stats.bytes_written += outcome.bytes_written;

            total_outcome.records_written += outcome.records_written;
            total_outcome.bytes_written += outcome.bytes_written;
        }
    }

    buffer.total_records = 0;
    buffer.total_bytes = 0;

    Ok(total_outcome)
}
