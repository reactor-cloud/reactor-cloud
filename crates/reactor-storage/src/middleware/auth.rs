//! Authentication middleware for reactor-storage.
//!
//! Extracts Bearer token and X-Reactor-Org header, resolves auth context,
//! and inserts StorageCtx into request extensions.
//!
//! Unlike reactor-data, storage supports anonymous access for public buckets.
//!
//! # Multi-tenancy support
//!
//! `StorageCtx` also contains an optional `TenantCtx` reference, which provides
//! project-scoped information in multi-tenant deployments. In single-tenant mode
//! (G2), this is the same for all requests. In multi-tenant mode (G3c), it's
//! resolved from the Host header or JWT claims.

use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use reactor_core::auth::{AuthCtx, OrgRef};
use reactor_core::id::{OrgId, UserId};
use reactor_core::TenantCtx;
use reactor_policy::PolicyEvalContext;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::state::StorageState;

/// Request context for storage operations.
///
/// Contains authentication context (user, org, permissions) and tenant context
/// (project scope). The tenant context is used for multi-tenant resource isolation.
#[derive(Debug, Clone)]
pub struct StorageCtx {
    /// Authenticated user context (None for anonymous).
    pub auth: Option<AuthCtx>,
    /// Tenant context for multi-tenancy support.
    ///
    /// In single-tenant mode, this is the same for all requests.
    /// In multi-tenant mode, this identifies the project scope.
    pub tenant: Option<Arc<TenantCtx>>,
    /// Request ID for tracing.
    pub request_id: Uuid,
    /// Whether this is an anonymous request.
    pub is_anonymous: bool,
}

impl StorageCtx {
    /// Create an authenticated storage context.
    pub fn authenticated(auth: AuthCtx, request_id: Uuid) -> Self {
        Self {
            auth: Some(auth),
            tenant: None,
            request_id,
            is_anonymous: false,
        }
    }

    /// Create an authenticated storage context with tenant.
    pub fn authenticated_with_tenant(auth: AuthCtx, tenant: Arc<TenantCtx>, request_id: Uuid) -> Self {
        Self {
            auth: Some(auth),
            tenant: Some(tenant),
            request_id,
            is_anonymous: false,
        }
    }

    /// Create an anonymous storage context.
    pub fn anonymous(request_id: Uuid) -> Self {
        Self {
            auth: None,
            tenant: None,
            request_id,
            is_anonymous: true,
        }
    }

    /// Create an anonymous storage context with tenant.
    pub fn anonymous_with_tenant(tenant: Arc<TenantCtx>, request_id: Uuid) -> Self {
        Self {
            auth: None,
            tenant: Some(tenant),
            request_id,
            is_anonymous: true,
        }
    }

    /// Set the tenant context.
    pub fn with_tenant(mut self, tenant: Arc<TenantCtx>) -> Self {
        self.tenant = Some(tenant);
        self
    }

    /// Get the tenant context.
    pub fn tenant(&self) -> Option<&TenantCtx> {
        self.tenant.as_deref()
    }

    /// Get the project ID from tenant context.
    pub fn project_id(&self) -> Option<reactor_core::ProjectId> {
        self.tenant.as_ref().map(|t| *t.project_id())
    }

    /// Get the user ID from the auth context.
    pub fn user_id(&self) -> Option<UserId> {
        self.auth.as_ref().and_then(|a| a.user_id())
    }

    /// Get the active organization ID.
    pub fn org_id(&self) -> Option<OrgId> {
        self.auth.as_ref().and_then(|a| a.active_org)
    }

    /// Check if user has a specific permission.
    pub fn has_permission(&self, permission: &str) -> bool {
        self.auth
            .as_ref()
            .map(|a| a.has_permission(permission))
            .unwrap_or(false)
    }

    /// Check if the context is authenticated.
    pub fn is_authenticated(&self) -> bool {
        self.auth.is_some()
    }
}

impl PolicyEvalContext for StorageCtx {
    fn user_id(&self) -> Option<UserId> {
        self.auth.as_ref().and_then(|a| a.user_id())
    }

    fn org_id(&self) -> Option<OrgId> {
        self.auth.as_ref().and_then(|a| a.active_org)
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.auth
            .as_ref()
            .map(|a| a.has_permission(permission))
            .unwrap_or(false)
    }

    fn email(&self) -> Option<&str> {
        self.auth.as_ref().and_then(|a| a.claims.email.as_deref())
    }

    fn session_id(&self) -> Option<&str> {
        None
    }
}

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
                code: "unauthorized".to_string(),
            }),
        )
    }

    fn forbidden(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: message.into(),
                code: "forbidden".to_string(),
            }),
        )
    }
}

/// Authentication middleware for storage.
///
/// Extracts the Bearer token and X-Reactor-Org header, resolves the auth context,
/// and inserts a `StorageCtx` into request extensions.
///
/// Unlike data middleware, this allows anonymous requests for public bucket access.
/// The route handlers are responsible for checking if anonymous access is allowed
/// for the specific bucket.
///
/// # Multi-tenancy
///
/// The middleware also extracts `TenantCtx` from extensions (if present, injected by
/// reactor-server's tenant middleware) and includes it in the `StorageCtx`. This
/// enables per-tenant resource isolation in multi-tenant deployments.
///
/// Admin token support: If config.admin_token is set and the request token matches,
/// creates a system context with full access (for internal service-to-service calls).
pub async fn auth_middleware(
    State(state): State<StorageState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Skip auth for health endpoint
    if path.ends_with("/health") {
        return next.run(request).await;
    }

    // Generate request ID
    let request_id = extract_or_generate_request_id(&request);

    // Extract TenantCtx from extensions (set by reactor-server's tenant middleware)
    let tenant_ctx = request.extensions().get::<Arc<TenantCtx>>().cloned();

    // Extract Bearer token (optional for storage)
    let token = extract_bearer_token(&request);

    // If no token provided, create anonymous context
    let mut ctx = if let Some(token) = token {
        // Check if this is an admin token (for internal service calls)
        if let Some(ref admin_token) = state.config.admin_token {
            if token == *admin_token {
                // Create system context with full access
                return create_system_context_and_continue(request, request_id, tenant_ctx, next)
                    .await;
            }
        }

        // Extract X-Reactor-Org header
        let org_ref = extract_org_ref(&request);

        // Resolve auth context
        match state.auth.resolve_ctx(&token, org_ref.as_ref()).await {
            Ok(auth_ctx) => StorageCtx::authenticated(auth_ctx, request_id),
            Err(e) => {
                tracing::warn!(error = %e, "auth context resolution failed");
                return AuthErrorResponse::forbidden(format!("authentication failed: {}", e))
                    .into_response();
            }
        }
    } else {
        // Anonymous request - allowed for public bucket reads
        StorageCtx::anonymous(request_id)
    };

    // Add tenant context if available
    if let Some(tenant) = tenant_ctx {
        ctx = ctx.with_tenant(tenant);
    }

    // Insert into extensions
    request.extensions_mut().insert(ctx);

    next.run(request).await
}

/// Create a system context with full access for admin token requests.
async fn create_system_context_and_continue(
    mut request: Request<Body>,
    request_id: Uuid,
    tenant_ctx: Option<Arc<TenantCtx>>,
    next: Next,
) -> Response {
    use reactor_core::auth::{AuthMethod, Claims};

    let nil_org = OrgId::from(Uuid::nil());

    // Create system claims with nil UUIDs
    let system_claims = Claims {
        sub: "system".to_string(),
        iss: "reactor".to_string(),
        aud: "reactor".to_string(),
        exp: i64::MAX, // Never expires
        iat: 0,
        nbf: None,
        email: None,
        amr: vec![AuthMethod::Apikey], // Use Apikey as system auth
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
        permissions: vec!["*".to_string()], // All permissions
    };

    let mut ctx = StorageCtx::authenticated(system_ctx, request_id);
    if let Some(tenant) = tenant_ctx {
        ctx = ctx.with_tenant(tenant);
    }
    request.extensions_mut().insert(ctx);

    next.run(request).await
}

/// Middleware that requires authentication (rejects anonymous requests).
pub async fn require_auth_middleware(
    State(state): State<StorageState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Skip auth for health endpoint
    if path.ends_with("/health") {
        return next.run(request).await;
    }

    // Generate request ID
    let request_id = extract_or_generate_request_id(&request);

    // Extract TenantCtx from extensions (set by reactor-server's tenant middleware)
    let tenant_ctx = request.extensions().get::<Arc<TenantCtx>>().cloned();

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
            // Create system context with full access
            return create_system_context_and_continue(request, request_id, tenant_ctx, next).await;
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

    // Construct StorageCtx with tenant
    let mut ctx = StorageCtx::authenticated(auth_ctx, request_id);
    if let Some(tenant) = tenant_ctx {
        ctx = ctx.with_tenant(tenant);
    }

    // Insert into extensions
    request.extensions_mut().insert(ctx);

    next.run(request).await
}

/// Extract Bearer token from Authorization header.
fn extract_bearer_token(request: &Request<Body>) -> Option<String> {
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

    // Try to parse as UUID first, otherwise treat as slug
    Some(
        header_str
            .parse()
            .unwrap_or_else(|_| OrgRef::Slug(header_str.to_string())),
    )
}

/// Extract X-Request-Id header or generate a new one.
fn extract_or_generate_request_id(request: &Request<Body>) -> Uuid {
    request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(Uuid::now_v7)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_bearer_token() {
        let mut request = Request::builder().body(Body::empty()).unwrap();

        // No header
        assert!(extract_bearer_token(&request).is_none());

        // Invalid format
        *request.headers_mut() = Default::default();
        request
            .headers_mut()
            .insert(header::AUTHORIZATION, HeaderValue::from_static("Basic xyz"));
        assert!(extract_bearer_token(&request).is_none());

        // Valid Bearer
        *request.headers_mut() = Default::default();
        request.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer my-token-123"),
        );
        assert_eq!(
            extract_bearer_token(&request),
            Some("my-token-123".to_string())
        );

        // Case insensitive
        *request.headers_mut() = Default::default();
        request.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("bearer another-token"),
        );
        assert_eq!(
            extract_bearer_token(&request),
            Some("another-token".to_string())
        );
    }

    #[test]
    fn test_extract_org_ref() {
        let mut request = Request::builder().body(Body::empty()).unwrap();

        // No header
        assert!(extract_org_ref(&request).is_none());

        // UUID
        *request.headers_mut() = Default::default();
        let uuid = Uuid::now_v7();
        request.headers_mut().insert(
            "x-reactor-org",
            HeaderValue::from_str(&uuid.to_string()).unwrap(),
        );
        let org_ref = extract_org_ref(&request).unwrap();
        assert!(matches!(org_ref, OrgRef::Id(_)));

        // Slug
        *request.headers_mut() = Default::default();
        request
            .headers_mut()
            .insert("x-reactor-org", HeaderValue::from_static("my-org-slug"));
        let org_ref = extract_org_ref(&request).unwrap();
        assert!(matches!(org_ref, OrgRef::Slug(s) if s == "my-org-slug"));
    }

    #[test]
    fn test_storage_ctx_anonymous() {
        let ctx = StorageCtx::anonymous(Uuid::now_v7());
        assert!(ctx.is_anonymous);
        assert!(!ctx.is_authenticated());
        assert!(ctx.user_id().is_none());
        assert!(ctx.org_id().is_none());
        assert!(!ctx.has_permission("any"));
    }
}
