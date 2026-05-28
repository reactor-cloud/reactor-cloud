//! Authentication middleware for reactor-data.
//!
//! Extracts Bearer token and X-Reactor-Org header, resolves auth context,
//! and inserts DataCtx into request extensions.

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
use reactor_policy::PolicyEvalContext;
use serde::Serialize;
use uuid::Uuid;

use crate::store::DataStore;
use crate::DataState;

/// Request context for data operations.
#[derive(Debug, Clone)]
pub struct DataCtx {
    /// Authenticated user context.
    pub auth: AuthCtx,
    /// Request ID for tracing.
    pub request_id: Uuid,
    /// Schema namespace for tables.
    pub schema: String,
}

impl DataCtx {
    /// Get the user ID from the auth context.
    pub fn user_id(&self) -> Option<UserId> {
        self.auth.user_id()
    }

    /// Get the active organization ID.
    pub fn org_id(&self) -> Option<OrgId> {
        self.auth.active_org
    }

    /// Check if user has a specific permission.
    pub fn has_permission(&self, permission: &str) -> bool {
        self.auth.has_permission(permission)
    }
}

impl PolicyEvalContext for DataCtx {
    fn user_id(&self) -> Option<UserId> {
        self.auth.user_id()
    }

    fn org_id(&self) -> Option<OrgId> {
        self.auth.active_org
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.auth.has_permission(permission)
    }

    fn email(&self) -> Option<&str> {
        self.auth.claims.email.as_deref()
    }

    fn session_id(&self) -> Option<&str> {
        // Session ID is a ReactorId in claims, we'd need to store the string representation
        // For now, return None - this can be improved if needed
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

/// Authentication middleware.
///
/// Extracts the Bearer token and X-Reactor-Org header, resolves the auth context,
/// and inserts a `DataCtx` into request extensions.
pub async fn auth_middleware<S: DataStore + Clone>(
    State(state): State<DataState<S>>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Skip auth for health endpoint
    if path.ends_with("/health") {
        return next.run(request).await;
    }

    // Extract Bearer token
    let token = match extract_bearer_token(&request) {
        Some(t) => t,
        None => {
            return AuthErrorResponse::unauthorized("missing or invalid Authorization header")
                .into_response()
        }
    };

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

    // Generate request ID
    let request_id = extract_or_generate_request_id(&request);

    // Construct DataCtx
    let ctx = DataCtx {
        auth: auth_ctx,
        request_id,
        schema: state.config.user_schema.clone(),
    };

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
}
