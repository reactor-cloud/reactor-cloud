//! Authentication middleware.

use crate::error::ConnectError;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::ConnectStore;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use reactor_core::auth::OrgRef;
use uuid::Uuid;

/// Extract bearer token from request.
pub fn extract_bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

/// Extract org header from request.
pub fn extract_org_header(req: &Request) -> Option<OrgRef> {
    req.headers()
        .get("X-Reactor-Org")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.parse::<OrgRef>().unwrap())
}

/// Auth middleware that verifies tokens and builds ConnectCtx.
pub async fn auth_middleware<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    mut req: Request,
    next: Next,
) -> Result<Response, ConnectError> {
    let token = extract_bearer_token(&req).ok_or(ConnectError::Unauthorized)?;
    let org_ref = extract_org_header(&req);

    // Resolve auth context
    let auth_ctx = state
        .auth
        .resolve_ctx(token, org_ref.as_ref())
        .await
        .map_err(|_| ConnectError::Unauthorized)?;

    // Get org ID - convert from ReactorId to Uuid
    let org_id = auth_ctx
        .active_org
        .ok_or(ConnectError::MissingOrgContext)?
        .into_uuid();

    // Generate request ID
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Convert user_id from ReactorId to Uuid
    let user_id = auth_ctx.user_id().map(|id| id.into_uuid());

    // Build context
    let ctx = ConnectCtx::new(request_id, org_id, user_id);

    // Insert context as extension
    req.extensions_mut().insert(ctx);

    Ok(next.run(req).await)
}

/// Extract ConnectCtx from request extensions.
pub fn extract_ctx(req: &Request) -> Result<&ConnectCtx, ConnectError> {
    req.extensions()
        .get::<ConnectCtx>()
        .ok_or(ConnectError::Unauthorized)
}
