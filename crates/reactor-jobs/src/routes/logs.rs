//! Logs streaming endpoint (SSE).

use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    Extension,
};
use futures::stream::{self, BoxStream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;

use crate::error::JobsError;
use crate::state::{JobCtx, JobsState};
use crate::store::{JobsStore, PgJobsStore, RunId};

/// Query parameters for logs endpoint.
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Filter by run ID.
    pub run_id: Option<RunId>,
    /// Follow logs (keep connection open).
    #[serde(default)]
    pub follow: bool,
    /// Maximum number of log entries to return.
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    100
}

/// Log entry sent via SSE.
#[derive(Debug, Serialize)]
pub struct LogEntry {
    /// Timestamp.
    pub ts: String,
    /// Log level.
    pub level: String,
    /// Run ID.
    pub run_id: String,
    /// Step name (if applicable).
    pub step: Option<String>,
    /// Log message.
    pub message: String,
}

/// Stream logs for a job via SSE.
///
/// GET /jobs/v1/_admin/jobs/{name}/logs
pub async fn stream_logs(
    State(state): State<JobsState>,
    Extension(ctx): Extension<JobCtx>,
    Path(name): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<Sse<BoxStream<'static, Result<Event, Infallible>>>, JobsError> {
    // Check permission
    let permission = format!("jobs:{}:logs", name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("*") {
        return Err(JobsError::PermissionDenied(permission));
    }

    let store = PgJobsStore::new(state.pool.clone());
    let org_id = (*ctx.active_org()).into_uuid();

    // Verify job exists
    let _job = store
        .get_job(org_id, &name)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(name.clone()))?;

    // For follow mode, we stream logs as they come in
    // For non-follow mode, we return recent logs and close
    let stream: BoxStream<'static, Result<Event, Infallible>> = if query.follow {
        // Polling stream - in production this would use a proper pub/sub
        stream::unfold(0u64, move |last_id| {
            async move {
                // Poll every 1 second for new logs
                tokio::time::sleep(Duration::from_secs(1)).await;

                // In a real implementation, this would query for logs since last_id
                // For now, return a heartbeat event
                let event = Event::default()
                    .event("heartbeat")
                    .data(format!("{{\"ts\":\"{}\"}}", chrono::Utc::now().to_rfc3339()));

                Some((Ok(event), last_id + 1))
            }
        })
        .boxed()
    } else {
        // Return recent logs and close
        // In production, we'd query from a logs table
        let initial_event = Event::default().event("connected").data(format!(
            "{{\"job\":\"{}\",\"ts\":\"{}\"}}",
            name,
            chrono::Utc::now().to_rfc3339()
        ));

        stream::once(async { Ok(initial_event) }).boxed()
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
