//! Tenant context middleware and extractor.
//!
//! In single-tenant mode (G2), the `TenantCtx` is injected via `axum::Extension`.
//! In multi-tenant mode (G3c), the middleware resolves tenant from Host header or JWT.
//!
//! # Single-tenant mode (current)
//!
//! The tenant context is built from config at startup and added as an Extension:
//!
//! ```ignore
//! let tenant_ctx = Arc::new(TenantCtx::new(...));
//! let app = Router::new()
//!     .layer(axum::Extension(tenant_ctx));
//! ```
//!
//! Handlers extract it via the [`Tenant`] extractor.
//!
//! # Multi-tenant mode (Phase 3+)
//!
//! In multi-tenant deployments, the `TenantProvider::HostLookup` variant resolves
//! the tenant from the Host header by looking up the routes table. Results are
//! cached with TTL and invalidated via LISTEN/NOTIFY.

use async_trait::async_trait;
use axum::{
    body::Body,
    extract::{FromRequestParts, State},
    http::{request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use moka::future::Cache;
use reactor_core::{ProjectId, ProjectRef, TenantCtx, TenantEnv};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};
use uuid::Uuid;

/// Cache entry for a resolved tenant.
#[derive(Clone)]
struct TenantCacheEntry {
    ctx: Arc<TenantCtx>,
}

/// Shared tenant context for single-tenant deployments.
///
/// In single-tenant mode, this is cloned for every request.
/// In multi-tenant mode, it's resolved dynamically from Host header.
#[derive(Clone)]
pub struct TenantProvider {
    inner: TenantProviderInner,
}

#[derive(Clone)]
enum TenantProviderInner {
    /// Fixed tenant context (single-tenant mode).
    Fixed(Arc<TenantCtx>),
    /// Dynamic resolution via Host header lookup (multi-tenant mode).
    HostLookup {
        cache: Cache<String, TenantCacheEntry>,
        pool: PgPool,
        fallback_host: Option<String>,
    },
}

impl TenantProvider {
    /// Create a fixed tenant provider for single-tenant mode.
    pub fn fixed(ctx: TenantCtx) -> Self {
        Self {
            inner: TenantProviderInner::Fixed(Arc::new(ctx)),
        }
    }

    /// Create from project configuration.
    pub fn from_config(
        project_id: ProjectId,
        project_ref: ProjectRef,
        project_name: String,
        env: TenantEnv,
    ) -> Self {
        let ctx = TenantCtx::new(project_id, project_ref, project_name, env);
        Self::fixed(ctx)
    }

    /// Create a default tenant provider for development.
    ///
    /// Uses a well-known nil project ID; should only be used for local dev.
    pub fn default_dev() -> Self {
        let project_id = ProjectId::nil();
        let project_ref = project_id.to_ref();
        let ctx = TenantCtx::new(project_id, project_ref, "dev", TenantEnv::Dev);
        Self::fixed(ctx)
    }

    /// Create a host-lookup tenant provider for multi-tenant mode.
    ///
    /// This provider resolves tenants from the Host header by looking up
    /// the routes table. Results are cached for the specified TTL.
    ///
    /// # Arguments
    /// * `pool` - Database connection pool
    /// * `ttl` - Cache TTL (e.g., 60 seconds)
    /// * `fallback_host` - Optional fallback host for requests without Host header
    pub fn host_lookup(pool: PgPool, ttl: Duration, fallback_host: Option<String>) -> Self {
        Self::host_lookup_with_capacity(pool, ttl, fallback_host, 10_000)
    }

    /// Create a host-lookup tenant provider with configurable cache capacity.
    ///
    /// Use this for Phase 4+ shared cluster deployments where thousands of tenants
    /// may be active concurrently.
    ///
    /// # Arguments
    /// * `pool` - Database connection pool
    /// * `ttl` - Cache TTL (e.g., 60 seconds)
    /// * `fallback_host` - Optional fallback host for requests without Host header
    /// * `max_capacity` - Maximum number of cached tenant contexts
    pub fn host_lookup_with_capacity(
        pool: PgPool,
        ttl: Duration,
        fallback_host: Option<String>,
        max_capacity: u64,
    ) -> Self {
        let cache = Cache::builder()
            .time_to_live(ttl)
            .max_capacity(max_capacity)
            .build();

        Self {
            inner: TenantProviderInner::HostLookup {
                cache,
                pool,
                fallback_host,
            },
        }
    }

    /// Get the tenant context for a request.
    ///
    /// In single-tenant mode, returns the fixed context.
    /// In multi-tenant mode, resolves from Host header.
    pub async fn resolve_async(&self, parts: &Parts) -> Result<Arc<TenantCtx>, TenantResolutionError> {
        match &self.inner {
            TenantProviderInner::Fixed(ctx) => Ok(ctx.clone()),
            TenantProviderInner::HostLookup { cache, pool, fallback_host } => {
                // Get host from X-Forwarded-Host (set by trusted proxy) or Host header
                let host = parts
                    .headers
                    .get("x-forwarded-host")
                    .or_else(|| parts.headers.get("host"))
                    .and_then(|v| v.to_str().ok())
                    .map(|h| h.split(':').next().unwrap_or(h)) // Strip port
                    .or(fallback_host.as_deref())
                    .ok_or_else(|| TenantResolutionError::InvalidHost("missing host header".to_string()))?
                    .to_string();

                // Check cache
                if let Some(entry) = cache.get(&host).await {
                    debug!(host = %host, "tenant cache hit");
                    return Ok(entry.ctx);
                }

                // Cache miss - look up in database
                debug!(host = %host, "tenant cache miss, querying database");
                let ctx = self.lookup_tenant(pool, &host).await?;

                // Cache the result
                let entry = TenantCacheEntry { ctx: ctx.clone() };
                cache.insert(host.clone(), entry).await;

                Ok(ctx)
            }
        }
    }

    /// Get the tenant context for a request (sync version for compatibility).
    ///
    /// In single-tenant mode, returns the fixed context.
    /// In multi-tenant mode, this method is not supported - use resolve_async instead.
    pub fn resolve(&self, _parts: &Parts) -> Result<Arc<TenantCtx>, TenantResolutionError> {
        match &self.inner {
            TenantProviderInner::Fixed(ctx) => Ok(ctx.clone()),
            TenantProviderInner::HostLookup { .. } => {
                Err(TenantResolutionError::NotConfigured)
            }
        }
    }

    /// Get the fixed tenant context (returns None if using host lookup).
    ///
    /// Safe to use in single-tenant mode where you know the context is fixed.
    pub fn fixed_ctx(&self) -> Option<&Arc<TenantCtx>> {
        match &self.inner {
            TenantProviderInner::Fixed(ctx) => Some(ctx),
            TenantProviderInner::HostLookup { .. } => None,
        }
    }

    /// Check if this is a host-lookup provider.
    pub fn is_host_lookup(&self) -> bool {
        matches!(self.inner, TenantProviderInner::HostLookup { .. })
    }

    /// Invalidate a cached tenant by host.
    ///
    /// Call this when receiving LISTEN/NOTIFY for route changes.
    pub async fn invalidate(&self, host: &str) {
        if let TenantProviderInner::HostLookup { cache, .. } = &self.inner {
            debug!(host = %host, "invalidating tenant cache entry");
            cache.invalidate(host).await;
        }
    }

    /// Clear the entire tenant cache.
    pub async fn clear_cache(&self) {
        if let TenantProviderInner::HostLookup { cache, .. } = &self.inner {
            debug!("clearing entire tenant cache");
            cache.invalidate_all();
        }
    }

    /// Look up a tenant from the routes table.
    async fn lookup_tenant(&self, pool: &PgPool, host: &str) -> Result<Arc<TenantCtx>, TenantResolutionError> {
        // Query the routes table for this host
        let row: Option<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT r.project_id, r.project_ref, p.name
            FROM reactor_gateway.routes r
            JOIN reactor_cloud.projects p ON p.id = r.project_id
            WHERE r.host = $1 AND r.enabled = true AND p.status = 'active'
            "#,
        )
        .bind(host)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            warn!(host = %host, error = %e, "database error during tenant lookup");
            TenantResolutionError::NotConfigured
        })?;

        match row {
            Some((project_id, project_ref, name)) => {
                let project_id = ProjectId::from(project_id);
                let project_ref = ProjectRef::from_string_unchecked(project_ref);
                let ctx = TenantCtx::new(project_id, project_ref, name, TenantEnv::Production);
                Ok(Arc::new(ctx))
            }
            None => {
                debug!(host = %host, "tenant not found");
                Err(TenantResolutionError::NotFound(host.to_string()))
            }
        }
    }
}

/// Error when tenant resolution fails.
#[derive(Debug, Clone)]
pub enum TenantResolutionError {
    /// Multi-tenant mode not configured.
    NotConfigured,
    /// Host header missing or invalid.
    #[allow(dead_code)]
    InvalidHost(String),
    /// Tenant not found.
    #[allow(dead_code)]
    NotFound(String),
}

impl std::fmt::Display for TenantResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "tenant context not available"),
            Self::InvalidHost(h) => write!(f, "invalid host header: {}", h),
            Self::NotFound(r) => write!(f, "tenant not found: {}", r),
        }
    }
}

impl std::error::Error for TenantResolutionError {}

impl IntoResponse for TenantResolutionError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotConfigured => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidHost(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
        };
        let body = serde_json::json!({
            "error": "tenant_resolution_failed",
            "message": self.to_string(),
        });
        (status, axum::Json(body)).into_response()
    }
}

/// Returns true if the given request path is platform-scoped and should
/// bypass tenant resolution.
///
/// Platform-scoped paths are not tied to a specific tenant — they serve the
/// cluster itself (health, metrics), the global identity service (`/auth/v1`),
/// the control plane (`/_admin`, `/_ops`, `/_cloud`, `/_internal`), and
/// well-known discovery URIs. Forcing a Host-based tenant lookup on these
/// would break Fly health checks (which use the machine IP as Host) and
/// prevent bootstrap (signup, operator promotion) before any tenant exists.
fn is_platform_path(path: &str) -> bool {
    /// Matches `prefix` or `prefix` followed by `/`.
    fn under(path: &str, prefix: &str) -> bool {
        path == prefix || path.starts_with(&format!("{prefix}/"))
    }

    matches!(path, "/health" | "/metrics")
        || under(path, "/_admin")
        || under(path, "/_ops")
        || under(path, "/_internal")
        || under(path, "/_cloud")
        || under(path, "/auth/v1")
        || path.starts_with("/.well-known/")
}

/// Returns true if the request carries the platform admin token.
///
/// Used to bypass tenant resolution for in-process loopback calls from the
/// deploy pipeline. The sites capability uploads static assets via an HTTP
/// loopback client (`http://0.0.0.0:8000/storage/v1/object/...`) authenticated
/// with the admin token; that Host header has no tenant route, so we treat
/// the admin token as proof the request is the cluster talking to itself.
fn has_admin_token(request: &Request<Body>, admin_token: &str) -> bool {
    if admin_token.is_empty() {
        return false;
    }
    request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t == admin_token)
        .unwrap_or(false)
}

/// State for the tenant resolution middleware.
///
/// Holds the [`TenantProvider`] used to look up tenants from request hosts,
/// plus the cluster admin token. The admin token is used to detect in-process
/// loopback calls (e.g. the deploy pipeline uploading site assets via HTTP)
/// so they can bypass tenant resolution without a Host-based route.
#[derive(Clone)]
pub struct TenantMiddlewareState {
    pub provider: Arc<TenantProvider>,
    pub admin_token: String,
}

/// Middleware that resolves tenant from host header and adds to request extensions.
///
/// This is the multi-tenant mode middleware. For single-tenant mode, the tenant
/// context is added via `axum::Extension` at startup.
///
/// Platform-scoped paths (see [`is_platform_path`]) bypass tenant resolution
/// entirely so they remain reachable on a freshly bootstrapped cluster and on
/// Fly health checks that use the machine IP as the Host header.
///
/// Requests carrying the platform admin token (loopback service-to-service
/// calls inside the same process) also bypass tenant resolution — see
/// [`has_admin_token`] for the rationale.
///
/// Routes that need tenant context can use the `Tenant` extractor.
pub async fn tenant_resolution_middleware(
    State(state): State<Arc<TenantMiddlewareState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Bypass tenant resolution for platform-scoped paths
    if is_platform_path(request.uri().path()) {
        return next.run(request).await;
    }

    // Bypass tenant resolution for in-process loopback calls authenticated
    // with the admin token (e.g. sites uploading static assets to storage).
    if has_admin_token(&request, &state.admin_token) {
        return next.run(request).await;
    }

    // Extract parts we need for tenant resolution
    let (mut parts, body) = request.into_parts();

    match state.provider.resolve_async(&parts).await {
        Ok(tenant_ctx) => {
            // Add tenant context to request extensions
            parts.extensions.insert(tenant_ctx);
            let request = Request::from_parts(parts, body);
            next.run(request).await
        }
        Err(e) => {
            warn!(error = %e, "tenant resolution failed");
            e.into_response()
        }
    }
}

/// Extractor for `TenantCtx` from request extensions.
///
/// In single-tenant mode, the `TenantCtx` is added via `axum::Extension`.
/// Handlers can extract it to get project-scoped information.
///
/// # Usage
///
/// ```ignore
/// async fn handler(tenant: Tenant) -> impl IntoResponse {
///     format!("Project: {}", tenant.project_name())
/// }
/// ```
#[derive(Clone)]
pub struct Tenant(pub Arc<TenantCtx>);

impl Tenant {
    /// Create a new Tenant wrapper.
    pub fn new(ctx: Arc<TenantCtx>) -> Self {
        Self(ctx)
    }

    /// Get the inner Arc<TenantCtx>.
    pub fn into_inner(self) -> Arc<TenantCtx> {
        self.0
    }
}

impl std::ops::Deref for Tenant {
    type Target = TenantCtx;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Tenant
where
    S: Send + Sync,
{
    type Rejection = TenantResolutionError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Arc<TenantCtx>>()
            .cloned()
            .map(Tenant)
            .ok_or(TenantResolutionError::NotConfigured)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_provider_fixed() {
        let id = ProjectId::new();
        let ref_ = id.to_ref();
        let ctx = TenantCtx::new(id, ref_, "Test Project", TenantEnv::Production);
        let provider = TenantProvider::fixed(ctx);

        let fixed = provider.fixed_ctx().unwrap();
        assert_eq!(fixed.project_name(), "Test Project");
    }

    #[test]
    fn test_tenant_provider_default_dev() {
        let provider = TenantProvider::default_dev();
        let fixed = provider.fixed_ctx().unwrap();
        assert_eq!(fixed.project_name(), "dev");
        assert_eq!(fixed.env(), TenantEnv::Dev);
    }

    #[test]
    fn test_tenant_deref() {
        let id = ProjectId::new();
        let ctx = TenantCtx::from_project_id(id, "Deref Test");
        let tenant = Tenant(Arc::new(ctx));
        assert_eq!(tenant.project_name(), "Deref Test");
    }

    #[test]
    fn test_is_platform_path_bypasses() {
        // Cluster health and metrics
        assert!(is_platform_path("/health"));
        assert!(is_platform_path("/metrics"));

        // Control plane
        assert!(is_platform_path("/_admin"));
        assert!(is_platform_path("/_admin/deploy"));
        assert!(is_platform_path("/_admin/vault/foo/bar"));
        assert!(is_platform_path("/_ops/v1/deployments"));
        assert!(is_platform_path("/_ops/v1/operators/bootstrap"));
        assert!(is_platform_path("/_internal/resolve_ctx"));
        assert!(is_platform_path("/_cloud/v1/projects"));

        // Global identity service
        assert!(is_platform_path("/auth/v1/signup"));
        assert!(is_platform_path("/auth/v1/login"));
        assert!(is_platform_path("/auth/v1/token"));
        assert!(is_platform_path("/auth/v1/operators/bootstrap"));
        assert!(is_platform_path("/auth/v1/webauthn/register/start"));
        assert!(is_platform_path("/auth/v1/keys"));

        // Well-known discovery
        assert!(is_platform_path("/.well-known/openid-configuration"));
    }

    #[test]
    fn test_is_platform_path_does_not_bypass_tenant_paths() {
        // Per-tenant capability endpoints must still resolve a tenant
        assert!(!is_platform_path("/data/objects"));
        assert!(!is_platform_path("/storage/v1/objects/foo"));
        assert!(!is_platform_path("/functions/v1/myfn"));
        assert!(!is_platform_path("/sites/v1/list"));
        assert!(!is_platform_path("/jobs/v1/queue"));
        assert!(!is_platform_path("/analytics/v1/events"));
        assert!(!is_platform_path("/realtime/v1/connect"));
        assert!(!is_platform_path("/connect/v1/pairs"));
        assert!(!is_platform_path("/"));
        assert!(!is_platform_path("/api/foo"));

        // Edge cases — paths that look similar but aren't platform paths
        assert!(!is_platform_path("/healthcheck"));
        assert!(!is_platform_path("/auth"));
        assert!(!is_platform_path("/auth/v2/foo"));
        assert!(!is_platform_path("/_admin_extra"));
        assert!(!is_platform_path("/_opsx"));
    }
}
