//! Sync run management endpoints.
//!
//! This module includes both public routes for viewing runs and internal
//! routes for executing syncs (invoked by reactor-jobs).

use crate::error::ConnectError;
use crate::protocol::ConfiguredCatalog;
use crate::sink::{ReactorDataSink, ReactorDataSinkConfig};
use crate::state::{ConnectCtx, ConnectState};
use crate::store::{ConnectStore, SyncRunRecord};
use crate::sync::{SyncExecutor, SyncOptions, SyncOutcome};
use axum::{
    body::Body,
    extract::{Extension, Path, Query, State},
    response::{sse::Event, Sse},
    Json,
};
use chrono::Utc;
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

/// Response for listing runs.
#[derive(Debug, Serialize)]
pub struct RunListResponse {
    /// List of runs.
    pub runs: Vec<SyncRunRecord>,
}

/// GET /connect/v1/connections/:name/runs
pub async fn list<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<Json<RunListResponse>, ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name))?;

    let runs = state.store.list_runs(&connection.id, 50).await?;

    Ok(Json(RunListResponse { runs }))
}

/// GET /connect/v1/connections/:name/runs/:run_id
pub async fn get<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path((name, run_id)): Path<(String, Uuid)>,
) -> Result<Json<SyncRunRecord>, ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name))?;

    let runs = state.store.list_runs(&connection.id, 1000).await?;
    let run = runs
        .into_iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| ConnectError::Internal("run not found".to_string()))?;

    Ok(Json(run))
}

/// Log line for SSE stream.
#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    /// Timestamp.
    pub timestamp: chrono::DateTime<Utc>,
    /// Log level.
    pub level: String,
    /// Log message.
    pub message: String,
    /// Optional context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

/// GET /connect/v1/connections/:name/runs/:run_id/logs
///
/// SSE stream for run logs. Proxies from reactor-jobs when available.
pub async fn logs_sse<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path((name, run_id)): Path<(String, Uuid)>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name))?;

    // Verify run exists
    let runs = state.store.list_runs(&connection.id, 1000).await?;
    let _run = runs
        .iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| ConnectError::Internal("run not found".to_string()))?;

    // TODO: Proxy SSE logs from reactor-jobs if jobs_run_id is set
    // For now, return a placeholder stream that completes
    let placeholder_stream = stream::once(async move {
        let log = LogLine {
            timestamp: Utc::now(),
            level: "info".to_string(),
            message: format!("Sync run {} started", run_id),
            context: None,
        };
        let data = serde_json::to_string(&log).unwrap_or_default();
        Ok::<_, Infallible>(Event::default().data(data))
    });

    Ok(Sse::new(placeholder_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    ))
}

/// Trigger sync request.
#[derive(Debug, Deserialize)]
pub struct TriggerSyncRequest {
    /// Force full refresh (ignore state).
    #[serde(default)]
    pub full_refresh: bool,
    /// Run in sandbox mode.
    #[serde(default)]
    pub sandbox: bool,
}

/// Trigger sync response.
#[derive(Debug, Serialize)]
pub struct TriggerSyncResponse {
    /// Run ID.
    pub run_id: Uuid,
    /// Status.
    pub status: String,
    /// Message.
    pub message: String,
}

/// POST /connect/v1/connections/:name/trigger
pub async fn trigger<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
    Json(req): Json<TriggerSyncRequest>,
) -> Result<Json<TriggerSyncResponse>, ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name.clone()))?;

    // Create a run record
    let run_id = Uuid::new_v4();
    let run = SyncRunRecord {
        id: run_id,
        connection_id: connection.id,
        org_id: connection.org_id,
        jobs_run_id: None,
        status: "pending".to_string(),
        records_read: serde_json::json!({}),
        records_written: serde_json::json!({}),
        error_code: None,
        error_message: None,
        error_suggested_fix: None,
        started_at: None,
        finished_at: None,
        created_at: Utc::now(),
    };

    state.store.record_run(&run).await?;

    // TODO: Dispatch to reactor-jobs or execute inline for manual triggers
    // For now, we'll execute inline for simplicity
    // In production, this would POST to /jobs/v1/_admin/jobs

    let message = if req.sandbox {
        format!("Sandbox sync triggered for connection '{}'", name)
    } else {
        format!("Sync triggered for connection '{}'", name)
    };

    Ok(Json(TriggerSyncResponse {
        run_id,
        status: "pending".to_string(),
        message,
    }))
}

// =============================================================================
// Internal Routes (invoked by reactor-jobs)
// =============================================================================

/// Internal execute request (from reactor-jobs).
#[derive(Debug, Deserialize)]
pub struct InternalExecuteRequest {
    /// Run ID.
    pub run_id: Uuid,
    /// Whether to force full refresh.
    #[serde(default)]
    pub full_refresh: bool,
    /// Whether this is a sandbox run.
    #[serde(default)]
    pub sandbox: bool,
}

/// Internal execute response.
#[derive(Debug, Serialize)]
pub struct InternalExecuteResponse {
    /// Sync outcome.
    pub outcome: SyncOutcome,
}

/// POST /connect/v1/_internal/runs/:connection_id/execute
///
/// This endpoint is invoked by reactor-jobs when a scheduled sync triggers.
/// It executes the actual sync and returns the outcome.
pub async fn internal_execute<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Path(connection_id): Path<Uuid>,
    Json(req): Json<InternalExecuteRequest>,
) -> Result<Json<InternalExecuteResponse>, ConnectError> {
    // Load connection
    let connection = state
        .store
        .get_connection_by_id(&connection_id)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(connection_id.to_string()))?;

    // Update run status to running
    // TODO: update_run_status method in store
    
    // Load source instance
    let source_instance_id = connection.source_instance_id
        .ok_or_else(|| ConnectError::Internal("connection has no source instance".to_string()))?;
    let source_instance = state
        .store
        .get_instance_by_id(&source_instance_id)
        .await?
        .ok_or_else(|| ConnectError::Internal("source instance not found".to_string()))?;

    // Build configured catalog from source config
    let stream_configs: Vec<crate::routes::connections::StreamConfig> = 
        serde_json::from_value(connection.source_config_json.clone())
            .unwrap_or_default();

    let configured_streams: Vec<crate::protocol::ConfiguredStream> = stream_configs
        .into_iter()
        .map(|sc| crate::protocol::ConfiguredStream {
            stream: sc.name,
            sync_mode: match sc.mode.as_str() {
                "incremental" => crate::descriptor::SyncMode::IncrementalAppend,
                _ => crate::descriptor::SyncMode::FullRefresh,
            },
            cursor_field: None,
            primary_key: sc.primary_key,
        })
        .collect();

    let catalog = ConfiguredCatalog {
        streams: configured_streams,
    };

    // Use instance config (credentials are fetched separately via vault in actual impl)
    // TODO: Integrate with vault for proper credential retrieval
    let config = source_instance.config_json.clone();

    // Create sync options
    let options = SyncOptions {
        sandbox: req.sandbox,
        max_records: if req.sandbox { Some(1000) } else { None },
        ..Default::default()
    };

    // Create executor and sink
    let executor = SyncExecutor::new(state.store.clone(), state.runtime.clone());
    
    // TODO: Support EphemeralSink for sandbox mode (requires pool access)
    let sink: Box<dyn crate::sink::DestinationSink> = 
        Box::new(ReactorDataSink::new(ReactorDataSinkConfig::default()));

    // Execute sync
    let outcome = executor
        .execute(&connection, &config, &catalog, sink.as_ref(), &options)
        .await?;

    // Record final run state
    let final_run = SyncRunRecord {
        id: req.run_id,
        connection_id: connection.id,
        org_id: connection.org_id,
        jobs_run_id: None,
        status: if outcome.success { "succeeded" } else { "failed" }.to_string(),
        records_read: serde_json::to_value(&outcome.stream_stats)
            .unwrap_or_else(|_| serde_json::json!({})),
        records_written: serde_json::to_value(&outcome.stream_stats)
            .unwrap_or_else(|_| serde_json::json!({})),
        error_code: outcome.error.as_ref().map(|e| e.code.clone()),
        error_message: outcome.error.as_ref().map(|e| e.message.clone()),
        error_suggested_fix: outcome.error.as_ref().and_then(|e| e.suggested_fix.clone()),
        started_at: Some(Utc::now() - chrono::Duration::milliseconds(outcome.duration_ms as i64)),
        finished_at: Some(Utc::now()),
        created_at: Utc::now(),
    };

    state.store.record_run(&final_run).await?;

    Ok(Json(InternalExecuteResponse { outcome }))
}
