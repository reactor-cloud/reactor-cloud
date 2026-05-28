//! Stream sync execution loop.

use crate::error::ConnectError;
use crate::protocol::{ConfiguredCatalog, ConnectorMessage, StateBundle, SyncLimits};
use crate::runtime::ConnectorRuntime;
use crate::store::{ConnectStore, Connection, SyncRunRecord};
use futures::StreamExt;
use std::sync::Arc;

/// Execute a sync run.
pub async fn execute_sync<S: ConnectStore>(
    runtime: Arc<dyn ConnectorRuntime>,
    store: &S,
    connection: &Connection,
    sink: &dyn super::destination::DestinationSink,
    limits: &SyncLimits,
) -> Result<SyncRunRecord, ConnectError> {
    let run_id = uuid::Uuid::now_v7();
    let started_at = chrono::Utc::now();
    
    tracing::info!(
        connection_id = %connection.id,
        run_id = %run_id,
        "Starting sync run"
    );

    // Load source instance
    let source_instance = match connection.source_instance_id {
        Some(id) => store
            .get_instance_by_id(&id)
            .await?
            .ok_or_else(|| ConnectError::InstanceNotFound(id.to_string()))?,
        None => {
            return Err(ConnectError::InvalidInput(
                "Connection has no source instance".to_string(),
            ));
        }
    };

    // Build configured catalog from connection config
    let catalog: ConfiguredCatalog = serde_json::from_value(connection.source_config_json.clone())
        .unwrap_or_else(|_| ConfiguredCatalog { streams: vec![] });

    // Load prior state
    let mut state_bundle = StateBundle::default();
    for stream in &catalog.streams {
        if let Some(state) = store.get_state(&connection.id, &stream.stream).await? {
            state_bundle.stream_states.insert(stream.stream.clone(), state.state_json);
        }
    }

    // Prepare destination
    let descriptor = runtime.descriptor(&source_instance.type_id).await?;
    sink.prepare(&descriptor.streams).await?;

    // Start read
    let protocol_state = crate::protocol::StateBundle {
        stream_states: state_bundle.stream_states.clone(),
        global_state: state_bundle.global_state.clone(),
    };
    let mut message_stream = runtime
        .read(
            &source_instance.type_id,
            &source_instance.config_json,
            &catalog,
            Some(&protocol_state),
            limits,
        )
        .await?;

    let mut records_read: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut records_written: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut batch: Vec<crate::protocol::AirbyteRecordMessage> = Vec::with_capacity(500);
    let mut error: Option<(String, String)> = None;

    while let Some(result) = message_stream.next().await {
        match result {
            Ok(msg) => match msg {
                ConnectorMessage::Record(record) => {
                    *records_read.entry(record.stream.clone()).or_default() += 1;
                    batch.push(record);

                    // Flush batch if full
                    if batch.len() >= 500 {
                        let written = sink.write_batch(&batch).await?;
                        for record in &batch {
                            *records_written.entry(record.stream.clone()).or_default() += 1;
                        }
                        batch.clear();
                        tracing::debug!(written = written, "Flushed batch");
                    }

                    // Check row limit
                    if let Some(max) = limits.max_rows {
                        let total: u64 = records_read.values().sum();
                        if total >= max {
                            tracing::info!(total = total, limit = max, "Row limit reached");
                            break;
                        }
                    }
                }
                ConnectorMessage::State(state_msg) => {
                    // Flush pending records first
                    if !batch.is_empty() {
                        let _ = sink.write_batch(&batch).await?;
                        for record in &batch {
                            *records_written.entry(record.stream.clone()).or_default() += 1;
                        }
                        batch.clear();
                    }

                    // Persist state
                    super::cursor::parse_state_message(&state_msg, &mut state_bundle);
                    for (stream, state) in &state_bundle.stream_states {
                        store.put_state(&connection.id, stream, state).await?;
                    }
                }
                ConnectorMessage::Log(log) => {
                    tracing::debug!(level = ?log.level, message = %log.message, "Connector log");
                }
                ConnectorMessage::Trace(trace) => {
                    if let Some(err) = trace.error {
                        error = Some(("trace_error".to_string(), err.message));
                        break;
                    }
                }
                _ => {}
            },
            Err(e) => {
                error = Some((e.code().to_string(), e.to_string()));
                break;
            }
        }
    }

    // Flush remaining
    if !batch.is_empty() {
        let _ = sink.write_batch(&batch).await?;
        for record in &batch {
            *records_written.entry(record.stream.clone()).or_default() += 1;
        }
    }

    // Commit
    sink.commit().await?;

    let finished_at = chrono::Utc::now();
    let (status, error_code, error_message) = match error {
        Some((code, msg)) => ("failed".to_string(), Some(code), Some(msg)),
        None => ("succeeded".to_string(), None, None),
    };

    let run = SyncRunRecord {
        id: run_id,
        connection_id: connection.id,
        org_id: connection.org_id,
        jobs_run_id: None,
        status,
        records_read: serde_json::to_value(&records_read)?,
        records_written: serde_json::to_value(&records_written)?,
        error_code,
        error_message,
        error_suggested_fix: None,
        started_at: Some(started_at),
        finished_at: Some(finished_at),
        created_at: chrono::Utc::now(),
    };

    store.record_run(&run).await?;

    tracing::info!(
        run_id = %run_id,
        status = %run.status,
        read = ?records_read,
        written = ?records_written,
        duration_ms = %(finished_at - started_at).num_milliseconds(),
        "Sync run complete"
    );

    Ok(run)
}
