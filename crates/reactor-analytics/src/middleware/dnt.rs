//! DNT (Do Not Track) detection middleware.
//!
//! Detects DNT: 1 and Sec-GPC: 1 headers and sets flags on the request context.
//! When honor_dnt is enabled in config and DNT is detected, ingestion endpoints
//! will silently drop events with a 204 response.

use axum::{
    body::Body,
    http::{HeaderMap, Request},
    middleware::Next,
    response::Response,
};

/// Check if DNT (Do Not Track) or Sec-GPC (Global Privacy Control) headers are set.
pub fn is_dnt_requested(headers: &HeaderMap) -> bool {
    // Check DNT header
    if let Some(dnt) = headers.get("dnt") {
        if let Ok(value) = dnt.to_str() {
            if value == "1" {
                return true;
            }
        }
    }

    // Check Sec-GPC header (Global Privacy Control)
    if let Some(gpc) = headers.get("sec-gpc") {
        if let Ok(value) = gpc.to_str() {
            if value == "1" {
                return true;
            }
        }
    }

    false
}

/// Extension marker for DNT status.
#[derive(Debug, Clone, Copy)]
pub struct DntStatus {
    /// Whether DNT or Sec-GPC is requested.
    pub requested: bool,
}

/// Middleware that detects DNT/Sec-GPC headers and adds status to extensions.
pub async fn dnt_detector_middleware(mut request: Request<Body>, next: Next) -> Response {
    let dnt_requested = is_dnt_requested(request.headers());

    request.extensions_mut().insert(DntStatus {
        requested: dnt_requested,
    });

    next.run(request).await
}
