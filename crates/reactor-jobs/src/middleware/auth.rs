//! Auth middleware for jobs routes.
//!
//! Extracts Bearer token and X-Reactor-Org header, validates via AuthClient,
//! and constructs JobCtx.

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use reactor_core::auth::OrgRef;
use uuid::Uuid;

use crate::error::JobsError;
use crate::state::{JobCtx, JobsState};

/// Auth middleware that requires a valid Bearer token and org context.
pub async fn require_auth(
    State(state): State<JobsState>,
    mut request: Request,
    next: Next,
) -> Result<Response, JobsError> {
    // Extract request ID or generate one
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::now_v7().to_string());

    // Extract Bearer token
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(JobsError::MissingOrgContext)?;

    // Extract org reference and convert to OrgRef
    let org_ref: Option<OrgRef> = request
        .headers()
        .get("x-reactor-org")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.parse().expect("OrgRef FromStr is infallible"));

    // Validate token and get auth context
    let auth = state
        .auth
        .resolve_ctx(token, org_ref.as_ref())
        .await
        .map_err(|_| JobsError::MissingOrgContext)?;

    // Jobs require an active org
    let org_id = auth.active_org.ok_or(JobsError::MissingOrgContext)?;

    // Build job context
    let ctx = JobCtx::new(auth, request_id, org_id);

    // Insert context as extension
    request.extensions_mut().insert(ctx);

    Ok(next.run(request).await)
}
