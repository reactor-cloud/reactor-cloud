//! Unified logs SSE endpoint.
//!
//! Streams function logs from all runtime types via Server-Sent Events.

use axum::{
    extract::{Extension, Path, Query, State},
    response::{sse::Event, IntoResponse, Sse},
};
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::pin::Pin;
use std::time::Duration;

use crate::{
    error::FunctionsError,
    state::{FunctionCtx, FunctionsState},
    store::{FunctionsStore, PgFunctionsStore},
};

/// Query parameters for the logs endpoint.
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Whether to follow (stream) logs.
    #[serde(default)]
    pub follow: bool,
    /// Deployment ID to filter logs (optional).
    pub deployment_id: Option<String>,
    /// Start time (ISO 8601).
    pub since: Option<String>,
    /// Maximum number of log entries to return.
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    1000
}

/// A single log event.
#[derive(Debug, Clone, Serialize)]
pub struct LogEvent {
    /// Timestamp (ISO 8601).
    pub ts: String,
    /// Log level.
    pub level: String,
    /// Deployment ID.
    pub deployment_id: String,
    /// Request ID (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Log message.
    pub message: String,
}

/// GET /fn/v1/_admin/functions/{name}/logs
///
/// Stream function logs via Server-Sent Events.
/// Query params:
/// - follow: bool - whether to stream logs continuously
/// - deployment_id: string - filter by deployment
/// - since: string - start time (ISO 8601)
/// - limit: int - max entries (default 1000)
pub async fn stream_logs(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(function_name): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:logs", function_name);
    if !ctx.has_permission(&permission)
        && !ctx.has_permission("functions:*:logs")
        && !ctx.has_permission(&format!("functions:{}:admin", function_name))
        && !ctx.has_permission("functions:*:admin")
    {
        return Err(FunctionsError::PermissionDenied(permission));
    }

    // Get the function
    let store = PgFunctionsStore::new(state.pool.clone());
    let function = store
        .get_function_by_name(ctx.active_org(), &function_name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(function_name.clone()))?;

    // Create the log stream
    // TODO: Implement actual log streaming from runtimes
    // For now, return a placeholder stream
    let stream = create_log_stream(function.name, query.follow);

    Ok(Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(30))
                .text("keep-alive"),
        ))
}

/// Create a log stream for a function.
fn create_log_stream(
    function_name: String,
    follow: bool,
) -> impl Stream<Item = Result<Event, Infallible>> {
    // TODO: Implement actual log streaming based on runtime type:
    // - WASM: capture stdout/stderr via tracing
    // - Bun: pipe subprocess stdout/stderr
    // - Lambda: CloudWatch Logs subscription filter -> Kinesis -> local buffer

    let initial_event = LogEvent {
        ts: chrono::Utc::now().to_rfc3339(),
        level: "info".to_string(),
        deployment_id: "unknown".to_string(),
        request_id: None,
        message: format!("Connected to log stream for function '{}'", function_name),
    };

    let initial = stream::once(async move {
        let data = serde_json::to_string(&initial_event).unwrap_or_default();
        Ok::<_, Infallible>(Event::default().data(data).event("log"))
    });

    if follow {
        // For follow mode, create an infinite stream with heartbeats
        let heartbeat_stream = stream::unfold(0u64, move |counter| async move {
            tokio::time::sleep(Duration::from_secs(5)).await;

            let event = LogEvent {
                ts: chrono::Utc::now().to_rfc3339(),
                level: "debug".to_string(),
                deployment_id: "system".to_string(),
                request_id: None,
                message: format!("Heartbeat #{}", counter),
            };

            let data = serde_json::to_string(&event).unwrap_or_default();
            Some((
                Ok::<_, Infallible>(Event::default().data(data).event("heartbeat")),
                counter + 1,
            ))
        });

        Box::pin(initial.chain(heartbeat_stream)) as Pin<Box<dyn Stream<Item = _> + Send>>
    } else {
        // For non-follow mode, just return the initial connection event
        Box::pin(initial) as Pin<Box<dyn Stream<Item = _> + Send>>
    }
}
