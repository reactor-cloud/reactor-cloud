//! Quota enforcement middleware for axum.
//!
//! This middleware checks per-tenant quotas before allowing requests through.
//! Rate-limited requests get a 429 response with a Retry-After header.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use reactor_core::TenantCtx;
use std::sync::Arc;
use tracing::{debug, warn};

use super::service::{QuotaCheckResult, QuotaService};
use super::QuotaExceededResponse;

/// Quota middleware state.
#[derive(Clone)]
pub struct QuotaMiddlewareState {
    /// The quota service.
    pub service: Arc<QuotaService>,
}

impl QuotaMiddlewareState {
    /// Create new quota middleware state.
    pub fn new(service: Arc<QuotaService>) -> Self {
        Self { service }
    }
}

/// Quota enforcement middleware.
///
/// Checks the request rate limit for the tenant before allowing the request
/// to proceed. Returns 429 Too Many Requests if rate-limited.
///
/// # Usage
///
/// ```ignore
/// let quota_state = QuotaMiddlewareState::new(quota_service);
///
/// let app = Router::new()
///     .route("/api/v1/*path", get(handler))
///     .layer(axum::middleware::from_fn_with_state(
///         quota_state,
///         quota_middleware,
///     ));
/// ```
pub async fn quota_middleware(
    State(state): State<QuotaMiddlewareState>,
    Extension(tenant): Extension<Arc<TenantCtx>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let project_id = tenant.project_id();

    // Determine if this is a dedicated (paid) tenant
    // For now, we assume all tenants in shared mode are free tier
    // In production, this would be looked up from tenant metadata
    let is_dedicated = false;

    // Check request rate limit
    let result = state.service.check_request(&project_id, is_dedicated);

    match result {
        QuotaCheckResult::Allowed => {
            // Request allowed, continue to handler
            next.run(request).await
        }
        QuotaCheckResult::RateLimited { retry_after_secs } => {
            warn!(
                project_id = %project_id,
                retry_after_secs = retry_after_secs,
                "request rate limited"
            );

            QuotaExceededResponse {
                error: "RATE_LIMITED",
                message: "Request rate limit exceeded. Please slow down.".to_string(),
                quota: "requests_per_minute",
                limit: state.service.limits_for(&project_id, is_dedicated).requests_per_minute,
                retry_after_secs,
            }
            .into_response()
        }
        QuotaCheckResult::ConcurrentFunctionsExceeded => {
            // This shouldn't happen in the request middleware
            // (concurrent functions are checked in the functions capability)
            warn!(project_id = %project_id, "unexpected concurrent functions exceeded in middleware");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal error",
            )
                .into_response()
        }
        QuotaCheckResult::StorageExceeded => {
            QuotaExceededResponse {
                error: "STORAGE_EXCEEDED",
                message: "Storage quota exceeded. Please delete some files or upgrade your plan.".to_string(),
                quota: "storage_gb",
                limit: (state.service.limits_for(&project_id, is_dedicated).storage_bytes / 1_073_741_824) as u32,
                retry_after_secs: 0,
            }
            .into_response()
        }
        QuotaCheckResult::BandwidthExceeded => {
            QuotaExceededResponse {
                error: "BANDWIDTH_EXCEEDED",
                message: "Monthly bandwidth quota exceeded. Please wait until next month or upgrade your plan.".to_string(),
                quota: "bandwidth_gb_per_month",
                limit: (state.service.limits_for(&project_id, is_dedicated).bandwidth_bytes_per_month / 1_073_741_824) as u32,
                retry_after_secs: 0,
            }
            .into_response()
        }
    }
}

/// Middleware layer for conditional quota enforcement.
///
/// Only applies quota checks in multi-tenant mode. In single-tenant mode,
/// the quota middleware is a no-op.
pub async fn conditional_quota_middleware(
    State(state): State<Option<QuotaMiddlewareState>>,
    Extension(tenant): Extension<Arc<TenantCtx>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match state {
        Some(quota_state) => {
            // Multi-tenant mode: check quotas
            quota_middleware(State(quota_state), Extension(tenant), request, next).await
        }
        None => {
            // Single-tenant mode: no quota check
            next.run(request).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quota::QuotaServiceConfig;
    use axum::{routing::get, Router};
    use reactor_core::{ProjectId, ProjectRef, TenantEnv};
    use tower::ServiceExt;

    fn test_tenant_ctx() -> Arc<TenantCtx> {
        let id = ProjectId::new();
        let ref_ = id.to_ref();
        Arc::new(TenantCtx::new(id, ref_, "test", TenantEnv::Dev))
    }

    async fn test_handler() -> &'static str {
        "OK"
    }

    #[tokio::test]
    async fn test_middleware_allows_request() {
        let service = Arc::new(QuotaService::new(QuotaServiceConfig::default()));
        let state = QuotaMiddlewareState::new(service);
        let tenant = test_tenant_ctx();

        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(axum::middleware::from_fn_with_state(state, quota_middleware))
            .layer(Extension(tenant));

        let response = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_middleware_rate_limits() {
        let mut config = QuotaServiceConfig::default();
        config.free_tier_limits.requests_per_minute = 1;

        let service = Arc::new(QuotaService::new(config));
        let state = QuotaMiddlewareState::new(service.clone());
        let tenant = test_tenant_ctx();

        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(axum::middleware::from_fn_with_state(state.clone(), quota_middleware))
            .layer(Extension(tenant.clone()));

        // First request should succeed
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Second request should be rate-limited
        let response = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
