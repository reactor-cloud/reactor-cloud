//! Public serve plane handler.
//!
//! This handler serves site content by:
//! 1. Resolving the Host header to a site and deployment
//! 2. Matching the request path against deployment routes
//! 3. Dispatching to static file storage, functions, or redirects

use crate::bundle::CacheRules;
use crate::dispatch::RouteDecision;
use crate::error::SitesError;
use crate::middleware::host_resolver::{HostResolver, ResolvedHost};
use crate::route::decision::RouteResolver;
use crate::state::SitesState;
use crate::store::{DeploymentFunction, PgSitesStore, SitesStore};
use axum::{
    body::Body,
    extract::State,
    http::{header, Request, Response, StatusCode},
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// System bucket name for site assets.
const SITES_BUCKET: &str = "_reactor_sites";

/// Serve plane handler.
///
/// Routes incoming requests to the appropriate site and deployment based on
/// the Host header, then dispatches to static files, functions, or redirects.
pub async fn serve_handler(
    State(state): State<SitesState>,
    request: Request<Body>,
) -> Result<Response<Body>, SitesError> {
    // Extract host header
    let host = request
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let path = request.uri().path().to_string();
    let method = request.method().clone();

    // Resolve host to site and deployment
    let store = Arc::new(PgSitesStore::new(state.pool.clone()));
    let resolver = HostResolver::new(store.clone(), state.config.preview_subdomain.clone());

    let resolved = match resolver.resolve(host).await? {
        Some(r) => r,
        None => return Ok(not_found_response()),
    };

    // Check deployment is ready
    if resolved.deployment.status != "ready" {
        tracing::warn!(
            deployment_id = %resolved.deployment.id,
            status = %resolved.deployment.status,
            "Deployment not ready"
        );
        return Ok(Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("Deployment not ready"))
            .unwrap());
    }

    tracing::debug!(
        site_id = %resolved.site.id,
        deployment_id = %resolved.deployment.id,
        host = %host,
        path = %path,
        "Resolved host to site"
    );

    // Load routes and functions for the deployment
    let routes = store.get_deployment_routes(&resolved.deployment.id).await?;
    let functions = store
        .get_deployment_functions(&resolved.deployment.id)
        .await?;
    let function_map = build_function_map(&functions);

    tracing::debug!(
        route_count = routes.len(),
        function_count = functions.len(),
        "Loaded deployment routes"
    );

    // Build route resolver and match the path
    let route_resolver = RouteResolver::from_routes(&routes, function_map).map_err(|e| {
        SitesError::Internal(format!("Failed to build route resolver: {}", e))
    })?;

    let decision = route_resolver.resolve(&path, method.as_str());
    
    tracing::debug!(
        path = %path,
        decision = ?decision,
        "Route decision"
    );

    // Dispatch based on route decision
    match decision {
        RouteDecision::StaticFile {
            storage_key, cache, ..
        } => serve_static(&state, &resolved, &storage_key, &cache).await,
        RouteDecision::Function {
            function_id: _,
            function_name,
            sub_path,
        } => serve_function(&state, &resolved, &function_name, &sub_path, request).await,
        RouteDecision::Redirect {
            location, status, ..
        } => Ok(redirect_response(status, &location)),
        RouteDecision::Prerender { storage_key, .. } => {
            // For now, just serve the prerendered content as static
            // Full ISR cache machinery is deferred
            serve_static(&state, &resolved, &storage_key, &CacheRules::default()).await
        }
        RouteDecision::NotFound => Ok(not_found_response()),
    }
}

/// Build a map from function name to function ID.
fn build_function_map(functions: &[DeploymentFunction]) -> HashMap<String, Uuid> {
    functions
        .iter()
        .map(|f| (f.role.clone(), f.function_id))
        .collect()
}

/// Serve a static file from storage with index.html fallback.
async fn serve_static(
    state: &SitesState,
    resolved: &ResolvedHost,
    storage_key: &str,
    cache: &CacheRules,
) -> Result<Response<Body>, SitesError> {
    // Build the full storage path: {deployment_id}/static/{key}
    let deployment_id = resolved.deployment.id;
    let trimmed_key = storage_key.trim_start_matches('/');
    let base_key = format!("{}/static/{}", deployment_id, trimmed_key);
    
    tracing::debug!(
        storage_key = %storage_key,
        trimmed_key = %trimmed_key,
        base_key = %base_key,
        "Serving static file"
    );

    // Try multiple paths with index.html fallback (like Vercel/Netlify):
    // 1. Exact path
    // 2. path/index.html (for directory-style routes)
    // 3. path.html (for extension-less routes)
    let paths_to_try = if storage_key.ends_with('/') || storage_key.is_empty() {
        vec![
            format!("{}index.html", base_key),
            base_key.clone(),
        ]
    } else if storage_key.contains('.') {
        // Has extension, try exact path first
        vec![base_key.clone()]
    } else {
        // No extension, try with index.html and .html fallbacks
        vec![
            base_key.clone(),
            format!("{}/index.html", base_key),
            format!("{}.html", base_key),
        ]
    };

    let mut last_error = None;
    for key in &paths_to_try {
        match state.storage.get_object(SITES_BUCKET, key).await {
            Ok(bytes) => {
                // Determine content type from the key
                let content_type = guess_content_type(key);

                // Build cache-control header
                let cache_control = cache.to_cache_control();

                // Build response
                let mut response = Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, content_type)
                    .header(header::CACHE_CONTROL, cache_control);

                // Add X-Robots-Tag: noindex for preview deployments
                if resolved.is_preview {
                    response = response.header("X-Robots-Tag", "noindex");
                }

                return Ok(response.body(Body::from(bytes)).unwrap());
            }
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        }
    }

    // If we tried all paths and none worked, return 404
    tracing::debug!(
        storage_key = %storage_key,
        paths_tried = ?paths_to_try,
        error = ?last_error,
        "Static file not found"
    );
    Ok(not_found_response())
}

/// Serve a function invocation via the functions client.
async fn serve_function(
    state: &SitesState,
    resolved: &ResolvedHost,
    function_name: &str,
    sub_path: &str,
    request: Request<Body>,
) -> Result<Response<Body>, SitesError> {
    use axum::body::to_bytes;
    use bytes::Bytes;
    use reqwest::header::HeaderMap as ReqwestHeaderMap;

    tracing::debug!(
        function_name = %function_name,
        sub_path = %sub_path,
        site = %resolved.site.name,
        "Invoking function"
    );

    // Extract method
    let method = match request.method().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    };

    // Convert headers (filtering out hop-by-hop headers)
    let mut headers = ReqwestHeaderMap::new();
    let hop_by_hop = ["connection", "keep-alive", "host", "transfer-encoding", "te", "upgrade"];
    for (name, value) in request.headers() {
        let name_str = name.as_str().to_lowercase();
        if !hop_by_hop.contains(&name_str.as_str()) {
            if let Ok(name) = reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()) {
                if let Ok(value) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
                    headers.insert(name, value);
                }
            }
        }
    }

    // Add site context headers
    headers.insert(
        reqwest::header::HeaderName::from_static("x-reactor-site"),
        reqwest::header::HeaderValue::from_str(&resolved.site.name)
            .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static("unknown")),
    );
    if resolved.is_preview {
        headers.insert(
            reqwest::header::HeaderName::from_static("x-reactor-preview"),
            reqwest::header::HeaderValue::from_static("true"),
        );
    }

    // Extract body
    let body_bytes = to_bytes(request.into_body(), 10 * 1024 * 1024)
        .await
        .map_err(|e| SitesError::Internal(format!("failed to read request body: {}", e)))?;

    // Invoke the function
    let response = state
        .functions
        .invoke(function_name, sub_path, method, headers, Bytes::from(body_bytes.to_vec()))
        .await?;

    // Build response
    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    
    let mut builder = Response::builder().status(status);
    
    // Copy response headers (filtering hop-by-hop)
    for (name, value) in response.headers() {
        let name_str = name.as_str().to_lowercase();
        if !hop_by_hop.contains(&name_str.as_str()) {
            if let Ok(name) = header::HeaderName::from_bytes(name.as_str().as_bytes()) {
                if let Ok(value) = header::HeaderValue::from_bytes(value.as_bytes()) {
                    builder = builder.header(name, value);
                }
            }
        }
    }

    // Read response body (for streaming, use bytes_stream with the "stream" feature)
    let body_bytes = response
        .bytes()
        .await
        .map_err(|e| SitesError::FunctionDispatchFailed(format!("failed to read response: {}", e)))?;

    Ok(builder.body(Body::from(body_bytes.to_vec())).unwrap())
}

/// Guess content type from file path using mime_guess.
fn guess_content_type(path: &str) -> &'static str {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    
    // Convert common MIME types to static strings for efficiency
    match mime.essence_str() {
        "text/html" => "text/html; charset=utf-8",
        "text/css" => "text/css; charset=utf-8",
        "text/javascript" | "application/javascript" => "application/javascript; charset=utf-8",
        "application/json" => "application/json; charset=utf-8",
        "text/plain" => "text/plain; charset=utf-8",
        "text/xml" | "application/xml" => "application/xml; charset=utf-8",
        "image/svg+xml" => "image/svg+xml",
        "image/png" => "image/png",
        "image/jpeg" => "image/jpeg",
        "image/gif" => "image/gif",
        "image/webp" => "image/webp",
        "image/avif" => "image/avif",
        "image/x-icon" | "image/vnd.microsoft.icon" => "image/x-icon",
        "font/woff" => "font/woff",
        "font/woff2" => "font/woff2",
        "application/pdf" => "application/pdf",
        "application/wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Build a 404 Not Found response.
fn not_found_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>404 Not Found</title></head>
<body>
<h1>Not Found</h1>
<p>The requested resource could not be found.</p>
</body>
</html>"#,
        ))
        .unwrap()
}

/// Build a redirect response.
fn redirect_response(status: u16, location: &str) -> Response<Body> {
    let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::FOUND);

    Response::builder()
        .status(status_code)
        .header(header::LOCATION, location)
        .body(Body::empty())
        .unwrap()
}
