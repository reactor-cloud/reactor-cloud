//! Middleware for the ops control surface.
//!
//! The middleware stack runs in this order:
//! 1. Network: source IP must match trusted_networks allowlist
//! 2. Identity: extract Bearer JWT, resolve auth context
//! 3. Scope: check required scope for the route
//! 4. Step-up: verify mfa_at for flagged scopes
//! 5. Audit: log the operation after handler completes

use crate::audit::{AuditLogger, AuditStatus, OpsAuditEntry};
use crate::config::OpsConfig;
use crate::error::OpsError;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use reactor_core::auth::{AuthClient, AuthCtx, Claims};
use reactor_core::id::UserId;
use std::net::SocketAddr;
use std::sync::Arc;

/// Route metadata for scope requirements.
#[derive(Debug, Clone)]
pub struct RouteMeta {
    /// Required scope for this route.
    pub required_scope: String,
    /// Action name for audit logging.
    pub action: String,
    /// Resource type for audit logging.
    pub resource_type: Option<String>,
}

impl RouteMeta {
    /// Create new route metadata.
    pub fn new(scope: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            required_scope: scope.into(),
            action: action.into(),
            resource_type: None,
        }
    }

    /// Set the resource type for audit logging.
    pub fn with_resource_type(mut self, resource_type: impl Into<String>) -> Self {
        self.resource_type = Some(resource_type.into());
        self
    }
}

/// Ops authentication context extracted from the request.
#[derive(Debug, Clone)]
pub struct OpsAuthCtx {
    /// The verified claims from the JWT.
    pub claims: Claims,
    /// The resolved auth context.
    pub auth_ctx: AuthCtx,
    /// The user ID of the operator.
    pub user_id: UserId,
    /// Whether step-up authentication was verified for this request.
    pub step_up_verified: bool,
}

/// State for the ops middleware.
#[derive(Clone)]
pub struct OpsMiddlewareState {
    /// Auth client for identity verification.
    pub auth: Arc<dyn AuthClient>,
    /// Ops configuration.
    pub config: Arc<OpsConfig>,
    /// Audit logger.
    pub audit: AuditLogger,
}

/// Network check middleware.
///
/// Verifies the source IP is in the trusted networks allowlist.
pub async fn network_check(
    State(config): State<Arc<OpsConfig>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, OpsError> {
    let ip = addr.ip();

    if !config.is_trusted_ip(&ip) {
        tracing::warn!(
            ip = %ip,
            "ops request from untrusted network"
        );
        return Err(OpsError::UntrustedNetwork);
    }

    Ok(next.run(request).await)
}

/// Identity check middleware.
///
/// Extracts and validates the Bearer JWT token, resolving the auth context.
pub async fn identity_check(
    State(auth): State<Arc<dyn AuthClient>>,
    mut request: Request,
    next: Next,
) -> Result<Response, OpsError> {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(OpsError::AuthenticationRequired)?;

    // Parse Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(OpsError::AuthenticationRequired)?;

    // Resolve auth context
    let auth_ctx = auth
        .resolve_ctx(token, None)
        .await
        .map_err(|_| OpsError::InvalidToken)?;

    // Extract user ID
    let user_id = auth_ctx.claims.user_id()
        .ok_or(OpsError::InvalidToken)?;

    // Build ops auth context
    let ops_ctx = OpsAuthCtx {
        claims: auth_ctx.claims.clone(),
        auth_ctx,
        user_id,
        step_up_verified: false, // Will be set by step-up middleware if needed
    };

    // Insert into request extensions
    request.extensions_mut().insert(ops_ctx);

    Ok(next.run(request).await)
}

/// Scope check middleware.
///
/// Verifies the user has the required scope for the route.
/// The required scope is read from the route's extensions (set via RouteMeta).
pub async fn scope_check(
    request: Request,
    next: Next,
) -> Result<Response, OpsError> {
    // Get route metadata
    let meta = request.extensions().get::<RouteMeta>().cloned();

    if let Some(meta) = meta {
        // Get ops auth context
        let ops_ctx = request
            .extensions()
            .get::<OpsAuthCtx>()
            .ok_or(OpsError::AuthenticationRequired)?;

        // Check if user has the required scope
        if !ops_ctx.claims.has_scope(&meta.required_scope) {
            tracing::warn!(
                user_id = %ops_ctx.user_id,
                required_scope = %meta.required_scope,
                "user missing required scope"
            );
            return Err(OpsError::MissingScope(meta.required_scope.clone()));
        }
    }

    Ok(next.run(request).await)
}

/// Step-up check middleware.
///
/// For routes that require step-up authentication, verifies that mfa_at
/// is within the configured window.
pub async fn step_up_check(
    State(config): State<Arc<OpsConfig>>,
    mut request: Request,
    next: Next,
) -> Result<Response, OpsError> {
    // Get route metadata
    let meta = request.extensions().get::<RouteMeta>().cloned();

    if let Some(meta) = &meta {
        // Check if this scope requires step-up
        if config.requires_step_up(&meta.required_scope) {
            // Get ops auth context
            let ops_ctx = request
                .extensions()
                .get::<OpsAuthCtx>()
                .ok_or(OpsError::AuthenticationRequired)?
                .clone();

            // Check mfa_at
            if ops_ctx.claims.requires_step_up(config.step_up_window_secs as i64) {
                tracing::warn!(
                    user_id = %ops_ctx.user_id,
                    scope = %meta.required_scope,
                    "step-up authentication required"
                );
                return Err(OpsError::StepUpRequired);
            }

            // Mark step-up as verified
            let mut ops_ctx = ops_ctx;
            ops_ctx.step_up_verified = true;
            request.extensions_mut().insert(ops_ctx);
        }
    }

    Ok(next.run(request).await)
}

/// Audit middleware.
///
/// Logs the operation to the audit log after the handler completes.
pub async fn audit_log(
    State(state): State<OpsMiddlewareState>,
    request: Request,
    next: Next,
) -> Response {
    // Extract audit info before running the handler
    let ops_ctx = request.extensions().get::<OpsAuthCtx>().cloned();
    let meta = request.extensions().get::<RouteMeta>().cloned();

    let actor_ip = request
        .headers()
        .get("x-forwarded-for")
        .or_else(|| request.headers().get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());

    let actor_user_agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Run the handler
    let response = next.run(request).await;

    // Determine status based on response
    let status = if response.status().is_success() {
        AuditStatus::Success
    } else if response.status() == StatusCode::FORBIDDEN
        || response.status() == StatusCode::UNAUTHORIZED
    {
        AuditStatus::Denied
    } else {
        AuditStatus::Error
    };

    // Log the audit entry (best effort - don't fail the request if logging fails)
    if let Some(meta) = meta {
        let entry = OpsAuditEntry {
            actor_user_id: ops_ctx.as_ref().map(|c| c.user_id),
            actor_ip,
            actor_user_agent,
            action: meta.action,
            scope_used: Some(meta.required_scope),
            resource_type: meta.resource_type,
            resource_id: None, // Would need to extract from response body
            status,
            details: serde_json::json!({}),
            step_up_used: ops_ctx.map(|c| c.step_up_verified).unwrap_or(false),
        };

        if let Err(e) = state.audit.log(&entry).await {
            tracing::error!(error = %e, "failed to log audit entry");
        }
    }

    response
}

/// Extractor for OpsAuthCtx.
#[async_trait::async_trait]
impl<S> axum::extract::FromRequestParts<S> for OpsAuthCtx
where
    S: Send + Sync,
{
    type Rejection = OpsError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<OpsAuthCtx>()
            .cloned()
            .ok_or(OpsError::AuthenticationRequired)
    }
}
