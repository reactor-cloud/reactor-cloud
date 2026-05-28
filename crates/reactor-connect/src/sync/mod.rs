//! Stream sync execution.
//!
//! This module implements the core sync loop:
//! 1. Read records from source (via ConnectorRuntime)
//! 2. Buffer records in batches
//! 3. Write batches to sink (DestinationSink)
//! 4. Persist state checkpoints
//! 5. Handle schema discovery and drift detection

mod executor;
mod loop_protection;
mod schema;

pub use executor::{SyncExecutor, SyncOptions, SyncOutcome};
pub use loop_protection::{LoopMarker, LoopProtection, LoopProtectionConfig};
pub use schema::{SchemaChange, SchemaDiff, detect_drift};

use crate::error::ConnectError;
use crate::protocol::{AirbyteRecordMessage, AirbyteStateMessage, AirbyteStateType};
use crate::protocol::StateBundle as RuntimeStateBundle;
use crate::store::{ConnectStore, ConnectionId, StateBundle as StoreStateBundle};
use crate::sink::DestinationSink;
use std::collections::HashMap;

/// Record buffer for batching writes.
#[derive(Debug, Default)]
pub struct RecordBuffer {
    /// Records per stream.
    pub streams: HashMap<String, Vec<AirbyteRecordMessage>>,
    /// Total record count.
    pub total_records: u64,
    /// Total bytes (approximate).
    pub total_bytes: u64,
}

impl RecordBuffer {
    /// Create a new empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a record to the buffer.
    pub fn push(&mut self, record: AirbyteRecordMessage) {
        let bytes = serde_json::to_string(&record.data)
            .map(|s| s.len() as u64)
            .unwrap_or(0);

        self.streams
            .entry(record.stream.clone())
            .or_default()
            .push(record);

        self.total_records += 1;
        self.total_bytes += bytes;
    }

    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.total_records == 0
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.streams.clear();
        self.total_records = 0;
        self.total_bytes = 0;
    }

    /// Get buffer size.
    pub fn len(&self) -> u64 {
        self.total_records
    }
}

/// State manager for persisting stream state.
pub struct StateManager<S: ConnectStore> {
    store: S,
    connection_id: ConnectionId,
    /// Pending state updates (not yet persisted).
    pending: HashMap<String, serde_json::Value>,
    /// Last persisted state per stream.
    persisted: HashMap<String, serde_json::Value>,
    /// Global state.
    global_state: Option<serde_json::Value>,
}

impl<S: ConnectStore> StateManager<S> {
    /// Create a new state manager.
    pub fn new(store: S, connection_id: ConnectionId) -> Self {
        Self {
            store,
            connection_id,
            pending: HashMap::new(),
            persisted: HashMap::new(),
            global_state: None,
        }
    }

    /// Load existing state from the store for configured streams.
    pub async fn load(&mut self, streams: &[String]) -> Result<(), ConnectError> {
        for stream_name in streams {
            if let Some(state_bundle) = self.store.get_state(&self.connection_id, stream_name).await? {
                // The store returns a StateBundle per stream
                self.persisted.insert(stream_name.clone(), state_bundle.state_json);
            }
        }
        // Also try to load global state
        if let Some(state_bundle) = self.store.get_state(&self.connection_id, "__global__").await? {
            self.global_state = Some(state_bundle.state_json);
        }
        Ok(())
    }

    /// Get state for a stream (returns persisted state merged with pending).
    pub fn get(&self, stream: &str) -> Option<&serde_json::Value> {
        self.pending.get(stream).or_else(|| self.persisted.get(stream))
    }

    /// Get all states as a RuntimeStateBundle for the runtime.
    pub fn as_bundle(&self) -> Option<RuntimeStateBundle> {
        if self.persisted.is_empty() && self.pending.is_empty() && self.global_state.is_none() {
            return None;
        }

        // Merge persisted and pending
        let mut stream_states = self.persisted.clone();
        for (k, v) in &self.pending {
            if k != "__global__" {
                stream_states.insert(k.clone(), v.clone());
            }
        }

        Some(RuntimeStateBundle {
            stream_states,
            global_state: self.global_state.clone(),
        })
    }

    /// Record a state checkpoint (not yet persisted).
    pub fn checkpoint(&mut self, state: &AirbyteStateMessage) {
        match state.state_type {
            AirbyteStateType::Stream => {
                if let Some(stream_state) = &state.stream {
                    let stream_name = stream_state.stream_descriptor.name.clone();
                    self.pending.insert(stream_name, stream_state.stream_state.clone());
                }
            }
            AirbyteStateType::Global => {
                if let Some(global) = &state.global {
                    self.global_state = Some(global.shared_state.clone());
                    self.pending.insert("__global__".to_string(), global.shared_state.clone());
                }
            }
            AirbyteStateType::Legacy => {
                if let Some(data) = &state.data {
                    self.pending.insert("__legacy__".to_string(), data.clone());
                }
            }
        }
    }

    /// Persist all pending state to the store.
    pub async fn persist(&mut self) -> Result<(), ConnectError> {
        for (stream_name, state_json) in self.pending.drain() {
            self.store
                .put_state(&self.connection_id, &stream_name, &state_json)
                .await?;
            if stream_name != "__global__" {
                self.persisted.insert(stream_name, state_json);
            }
        }
        Ok(())
    }
}

/// Flush the buffer to the sink.
pub async fn flush_buffer(
    buffer: &mut RecordBuffer,
    sink: &dyn DestinationSink,
) -> Result<(), ConnectError> {
    for (stream, records) in buffer.streams.drain() {
        if !records.is_empty() {
            sink.write_batch(&stream, &records).await?;
        }
    }
    buffer.total_records = 0;
    buffer.total_bytes = 0;
    Ok(())
}
