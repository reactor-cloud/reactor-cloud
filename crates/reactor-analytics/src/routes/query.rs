//! Query endpoint.

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::error::AnalyticsError;
use crate::query::{QueryKind, QueryRequest};
use crate::state::{AnalyticsCtx, AnalyticsState};
use crate::store::AnalyticsStore;

/// Execute an analytics query.
///
/// POST /analytics/v1/query
pub async fn query<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<QueryRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Check base permission
    if !ctx.has_permission("analytics:query") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:query".to_string(),
        ));
    }

    // Check query-kind specific permissions
    let required_permission = match req.kind {
        QueryKind::Events => "analytics:query:events",
        QueryKind::Aggregate => "analytics:query:aggregate",
        QueryKind::Funnel => "analytics:query:funnel",
        QueryKind::Retention => "analytics:query:retention",
        QueryKind::Breakdown => "analytics:query:breakdown",
        QueryKind::Path => "analytics:query:path",
    };

    // Allow if they have wildcard or specific permission
    if !ctx.has_permission("*")
        && !ctx.has_permission("analytics:query:*")
        && !ctx.has_permission(required_permission)
    {
        return Err(AnalyticsError::Forbidden(format!(
            "missing permission: {required_permission}"
        )));
    }

    // Ensure query is for the correct project
    if req.project_id != ctx.project_id {
        return Err(AnalyticsError::Forbidden(
            "project_id mismatch".to_string(),
        ));
    }

    // Execute query
    let result = state.store.execute_query(&req, &ctx).await?;

    Ok((StatusCode::OK, Json(result)))
}
