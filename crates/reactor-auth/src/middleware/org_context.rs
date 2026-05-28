//! Org context middleware — resolves X-Reactor-Org header.

use axum::{body::Body, extract::Request, http::header::HeaderValue, response::Response};
use reactor_core::auth::OrgRef;
use reactor_core::id::OrgId;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Header name for explicit org context.
pub const ORG_HEADER: &str = "x-reactor-org";

/// Extracted org context from request.
///
/// Can hold either a resolved OrgId (for backward compatibility with routes
/// that don't need slug resolution) or an OrgRef (UUID or slug).
#[derive(Debug, Clone)]
pub struct OrgContext {
    /// The active organization ID (resolved, if available).
    pub org_id: Option<OrgId>,
    /// The raw organization reference (UUID or slug).
    pub org_ref: Option<OrgRef>,
    /// Whether this was explicitly set via header.
    pub explicit: bool,
}

impl OrgContext {
    /// Create a new org context with a resolved OrgId.
    pub fn new(org_id: Option<OrgId>, explicit: bool) -> Self {
        Self {
            org_id,
            org_ref: org_id.map(OrgRef::Id),
            explicit,
        }
    }

    /// Create a new org context with an unresolved OrgRef.
    pub fn with_ref(org_ref: Option<OrgRef>, explicit: bool) -> Self {
        let org_id = org_ref.as_ref().and_then(|r| r.as_id().copied());
        Self {
            org_id,
            org_ref,
            explicit,
        }
    }

    /// Create from the X-Reactor-Org header value.
    ///
    /// Parses the value as an OrgRef (tries UUID first, falls back to slug).
    pub fn from_header(value: &HeaderValue) -> Option<Self> {
        let s = value.to_str().ok()?;
        let org_ref: OrgRef = s.parse().ok()?;
        Some(Self::with_ref(Some(org_ref), true))
    }
}

/// Layer for OrgContext middleware.
#[derive(Clone)]
pub struct OrgContextLayer;

impl<S> Layer<S> for OrgContextLayer {
    type Service = OrgContextMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OrgContextMiddleware { inner }
    }
}

/// Middleware that extracts org context from headers.
#[derive(Clone)]
pub struct OrgContextMiddleware<S> {
    inner: S,
}

impl<S> Service<Request> for OrgContextMiddleware<S>
where
    S: Service<Request, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        // Try to extract org context from header (parses as OrgRef)
        let org_ctx = req
            .headers()
            .get(ORG_HEADER)
            .and_then(OrgContext::from_header)
            .unwrap_or_else(|| OrgContext::with_ref(None, false));

        // Store in extensions for later extraction
        req.extensions_mut().insert(org_ctx);

        self.inner.call(req)
    }
}
