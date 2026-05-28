//! Request ID middleware — adds X-Request-Id to requests/responses.

use axum::{
    body::Body,
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    response::Response,
};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use uuid::Uuid;

/// Header name for request ID.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Layer for RequestId middleware.
#[derive(Clone)]
pub struct RequestIdLayer;

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdMiddleware { inner }
    }
}

/// Middleware that assigns and propagates request IDs.
#[derive(Clone)]
pub struct RequestIdMiddleware<S> {
    inner: S,
}

impl<S> Service<Request> for RequestIdMiddleware<S>
where
    S: Service<Request, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        // Get existing request ID or generate new one
        let request_id = req
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Set header on request (might already exist)
        let header_name = HeaderName::from_static(REQUEST_ID_HEADER);
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            req.headers_mut().insert(header_name.clone(), value);
        }

        // Store in extensions for logging
        req.extensions_mut().insert(RequestId(request_id.clone()));

        let mut inner = self.inner.clone();
        let id = request_id;

        Box::pin(async move {
            let mut response = inner.call(req).await?;

            // Add request ID to response headers
            if let Ok(value) = HeaderValue::from_str(&id) {
                response.headers_mut().insert(REQUEST_ID_HEADER, value);
            }

            Ok(response)
        })
    }
}

/// Request ID extension for accessing in handlers.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);
