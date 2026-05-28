//! Logs SSE endpoint.

use crate::error::SitesError;
use crate::state::{SiteCtx, SitesState};
use crate::store::{PgSitesStore, SitesStore};
use axum::{
    body::Body,
    extract::{Extension, Path, Query, State},
    response::Response,
};
use serde::Deserialize;

/// Logs query parameters.
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Follow logs in real-time.
    #[serde(default)]
    pub follow: bool,
    /// Limit number of log entries.
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    200
}

/// Stream logs for a site.
pub async fn stream_logs(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<Response<Body>, SitesError> {
    let perm = format!("sites:{}:logs", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let _site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    if query.follow {
        let stream = futures::stream::unfold(0, |state| async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            let event = format!(
                "event: log\ndata: {{\"ts\": \"{}\", \"level\": \"info\", \"source\": \"router\", \"message\": \"heartbeat\"}}\n\n",
                chrono::Utc::now().to_rfc3339()
            );

            Some((Ok::<_, std::convert::Infallible>(event), state + 1))
        });

        let body = Body::from_stream(stream);

        Ok(Response::builder()
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(body)
            .unwrap())
    } else {
        let logs = serde_json::json!({
            "logs": [],
            "total": 0,
        });

        Ok(Response::builder()
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&logs).unwrap()))
            .unwrap())
    }
}
