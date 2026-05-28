//! Authentication middleware for analytics.
//!
//! Extracts Bearer token and X-Reactor-Project header, resolves auth context,
//! and inserts AnalyticsCtx into request extensions.
//!
//! Admin operations require authenticated context. Ingestion supports both
//! authenticated (via Bearer token) and anonymous (via project key) modes.

use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use reactor_core::auth::{AuthCtx, AuthMethod, Claims, OrgRef};
use reactor_core::id::OrgId;
use serde::Serialize;
use uuid::Uuid;

use crate::state::{AnalyticsCtx, AnalyticsState};
use crate::store::AnalyticsStore;

/// Error response for authentication failures.
#[derive(Debug, Serialize)]
struct AuthErrorResponse {
    error: String,
    code: String,
}

impl AuthErrorResponse {
    fn unauthorized(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::UNAUTHORIZED,
            Json(Self {
                error: message.into(),
                code: "analytics.unauthorized".to_string(),
            }),
        )
    }

    fn forbidden(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: message.into(),
                code: "analytics.forbidden".to_string(),
            }),
        )
    }
}

/// Authentication middleware for analytics admin endpoints.
///
/// Requires Bearer token and X-Reactor-Project header.
/// Returns 401 if authentication fails.
pub async fn require_auth_middleware<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Skip auth for health endpoint
    if path.ends_with("/health") || path.ends_with("/openapi.json") {
        return next.run(request).await;
    }

    // Generate request ID
    let request_id = extract_or_generate_request_id(&request);

    // Extract Bearer token (required)
    let token = match extract_bearer_token(&request) {
        Some(t) => t,
        None => {
            return AuthErrorResponse::unauthorized("missing or invalid Authorization header")
                .into_response()
        }
    };

    // Check if this is an admin token (for internal service calls)
    if let Some(ref admin_token) = state.config.admin_token {
        if token == *admin_token {
            return create_system_context_and_continue(request, request_id, next).await;
        }
    }

    // Extract X-Reactor-Org header
    let org_ref = extract_org_ref(&request);

    // Resolve auth context
    let auth_ctx = match state.auth.resolve_ctx(&token, org_ref.as_ref()).await {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::warn!(error = %e, "auth context resolution failed");
            return AuthErrorResponse::forbidden(format!("authentication failed: {}", e))
                .into_response();
        }
    };

    // Extract X-Reactor-Project header (required for analytics)
    let project_id = match extract_project_id(&request) {
        Some(id) => id,
        None => {
            return AuthErrorResponse::unauthorized("missing or invalid X-Reactor-Project header")
                .into_response()
        }
    };

    // Get the org_id from the auth context
    let org_id = auth_ctx
        .active_org
        .unwrap_or_else(|| OrgId::from(Uuid::nil()));

    // Construct AnalyticsCtx
    let ctx = AnalyticsCtx::authenticated(auth_ctx, project_id, org_id, request_id);

    // Insert into extensions
    request.extensions_mut().insert(ctx);

    next.run(request).await
}

/// Create a system context with full access for admin token requests.
async fn create_system_context_and_continue(
    mut request: Request<Body>,
    request_id: Uuid,
    next: Next,
) -> Response {
    let nil_org = OrgId::from(Uuid::nil());

    // Create system claims with nil UUIDs
    let system_claims = Claims {
        sub: "system".to_string(),
        iss: "reactor".to_string(),
        aud: "reactor".to_string(),
        exp: i64::MAX,
        iat: 0,
        nbf: None,
        email: None,
        amr: vec![AuthMethod::Apikey],
        orgs: vec![nil_org],
        session_id: None,
        default_org: Some(nil_org),
        scopes: vec![],
        mfa_at: None,
    };

    // Create a system auth context with full access
    let system_ctx = AuthCtx {
        claims: system_claims,
        active_org: Some(nil_org),
        permissions: vec!["*".to_string()],
    };

    // Extract project_id (use nil if not provided for system calls)
    let project_id = extract_project_id(&request).unwrap_or_else(Uuid::nil);

    let ctx = AnalyticsCtx::authenticated(system_ctx, project_id, nil_org, request_id);
    request.extensions_mut().insert(ctx);

    next.run(request).await
}

/// Extract Bearer token from Authorization header.
pub fn extract_bearer_token(request: &Request<Body>) -> Option<String> {
    let header_value = request.headers().get(header::AUTHORIZATION)?;
    let header_str = header_value.to_str().ok()?;

    if header_str.len() > 7 && header_str[..7].eq_ignore_ascii_case("Bearer ") {
        Some(header_str[7..].to_string())
    } else {
        None
    }
}

/// Extract X-Reactor-Org header and parse into OrgRef.
fn extract_org_ref(request: &Request<Body>) -> Option<OrgRef> {
    let header_value = request.headers().get("x-reactor-org")?;
    let header_str = header_value.to_str().ok()?;

    if header_str.is_empty() {
        return None;
    }

    Some(
        header_str
            .parse()
            .unwrap_or_else(|_| OrgRef::Slug(header_str.to_string())),
    )
}

/// Extract X-Reactor-Project header and parse as UUID.
pub fn extract_project_id(request: &Request<Body>) -> Option<Uuid> {
    let header_value = request.headers().get("x-reactor-project")?;
    let header_str = header_value.to_str().ok()?;
    header_str.parse().ok()
}

/// Extract X-Request-Id header or generate a new one.
pub fn extract_or_generate_request_id(request: &Request<Body>) -> Uuid {
    request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(Uuid::now_v7)
}
