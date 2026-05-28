//! Authentication middleware for admin routes.

use crate::error::SitesError;
use crate::state::{SiteCtx, SitesState};
use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
};
use reactor_core::auth::OrgRef;

/// Auth middleware for admin routes.
///
/// Extracts Bearer token and X-Reactor-Org header, validates with auth service,
/// and inserts SiteCtx into request extensions.
pub async fn auth_middleware(
    State(state): State<SitesState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, SitesError> {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(SitesError::AuthRequired)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(SitesError::InvalidToken)?;

    let org_ref = request
        .headers()
        .get("x-reactor-org")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.parse::<OrgRef>().unwrap());

    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let auth_ctx = state
        .auth
        .resolve_ctx(token, org_ref.as_ref())
        .await
        .map_err(|e| {
            tracing::debug!("token verification failed: {}", e);
            SitesError::InvalidToken
        })?;

    let site_ctx = SiteCtx::from_auth(auth_ctx, request_id)?;

    request.extensions_mut().insert(site_ctx);

    Ok(next.run(request).await)
}
