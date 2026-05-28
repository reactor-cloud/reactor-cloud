//! Destination sinks for stream data.
//!
//! Sinks are where stream data goes after being read from a connector.
//! Available sinks:
//! - ReactorDataSink: Posts records to reactor-data via HTTP
//! - ReactorStorageSink: Uploads records to reactor-storage via HTTP
//! - EphemeralSink: Stores records in an ephemeral _sandbox_* schema for testing
//! - ConnectorSink: Writes records to another connector (outbound/reverse sync)

mod connector;
mod ephemeral;
mod reactor_data;
mod reactor_storage;

pub use connector::{ConnectorSink, ConnectorSinkConfig};
pub use ephemeral::{EphemeralSink, EphemeralSinkConfig};
pub use reactor_data::{ReactorDataSink, ReactorDataSinkConfig};
pub use reactor_storage::{ReactorStorageSink, ReactorStorageSinkConfig, StorageFormat};

use crate::error::ConnectError;
use crate::protocol::{AirbyteRecordMessage, AirbyteStateMessage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Outcome of a sink write batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkWriteOutcome {
    /// Number of records written.
    pub records_written: u64,
    /// Bytes written.
    pub bytes_written: u64,
    /// Per-stream stats.
    pub stream_stats: HashMap<String, StreamStats>,
}

/// Per-stream write statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamStats {
    /// Records written for this stream.
    pub records: u64,
    /// Bytes written for this stream.
    pub bytes: u64,
}

/// Destination sink trait.
///
/// A sink receives batches of records from a stream sync and writes them
/// to a destination (reactor-data, reactor-storage, ephemeral schema, etc.).
#[async_trait]
pub trait DestinationSink: Send + Sync {
    /// Initialize the sink for a sync run.
    ///
    /// Called once at the start of a sync run. The sink can set up any
    /// required state (create tables, initialize connections, etc.).
    async fn init(
        &self,
        org_id: &Uuid,
        connection_id: &Uuid,
        streams: &[String],
    ) -> Result<(), ConnectError>;

    /// Write a batch of records.
    ///
    /// Records are grouped by stream for efficiency. The sink should
    /// handle retries internally for transient failures.
    async fn write_batch(
        &self,
        stream: &str,
        records: &[AirbyteRecordMessage],
    ) -> Result<SinkWriteOutcome, ConnectError>;

    /// Checkpoint state.
    ///
    /// Called when the connector emits a state message. The sink should
    /// ensure all previously written records are durable before returning.
    async fn checkpoint(&self, state: &AirbyteStateMessage) -> Result<(), ConnectError>;

    /// Finalize the sync.
    ///
    /// Called at the end of a successful sync run. The sink can clean up
    /// temporary state, promote staged data, etc.
    async fn finalize(&self) -> Result<SinkWriteOutcome, ConnectError>;

    /// Abort the sync.
    ///
    /// Called if the sync fails. The sink should roll back any partial
    /// writes and clean up state.
    async fn abort(&self, reason: &str) -> Result<(), ConnectError>;
}

/// A boxed destination sink.
pub type BoxedSink = Box<dyn DestinationSink>;
