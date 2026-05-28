//! Function invocation endpoint.
//!
//! Routes: `* /fn/v1/{name}` and `* /fn/v1/{name}/{*rest}`
//!
//! Handles request streaming, body size limits, timeouts, concurrency control,
//! and response shaping with platform headers.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::Response,
    Extension,
};
use futures::stream::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

use crate::bundle::RuntimeKind;
use crate::error::FunctionsError;
use crate::runtime::{IncomingRequest, InvokeResult};
use crate::state::{FunctionCtx, FunctionsState};
use crate::store::FunctionsStore;


/// Synthesized auth context header.
#[derive(Debug, Serialize)]
struct ReactorAuthHeader {
    user_id: Option<String>,
    org_id: String,
    permissions: Vec<String>,
}

/// Invoke handler for `* /fn/v1/{name}` and `* /fn/v1/{name}/{*rest}`.
pub async fn invoke_handler(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(params): Path<InvokeParams>,
    method: Method,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response, FunctionsError> {
    let start = Instant::now();
    let name = &params.name;
    let subpath = params.rest.as_deref().unwrap_or("");

    // Check invoke permission
    let permission = format!("functions:{}:invoke", name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:invoke") {
        return Err(FunctionsError::PermissionDenied(permission));
    }

    // Get the function
    let store = crate::store::PgFunctionsStore::new(state.pool.clone());
    let function = store
        .get_function_by_name(ctx.active_org(), name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(name.clone()))?;

    // Check if function has a current deployment
    let deployment_id = function
        .current_deployment_id
        .ok_or(FunctionsError::DeploymentNotReady)?;

    // Get the deployment
    let deployment = store
        .get_deployment(deployment_id)
        .await?
        .ok_or(FunctionsError::DeploymentNotReady)?;

    // Check deployment status
    if deployment.status != "ready" {
        return Err(FunctionsError::DeploymentNotReady);
    }

    // Parse manifest from deployment to get limits and concurrency settings
    let manifest: crate::bundle::Manifest = serde_json::from_value(deployment.manifest_json.clone())
        .map_err(|e| FunctionsError::Internal(format!("invalid manifest in deployment: {}", e)))?;

    // Get the runtime
    let runtime_kind: RuntimeKind = function
        .runtime
        .parse()
        .map_err(|_| FunctionsError::UnsupportedRuntime(function.runtime.clone()))?;

    let runtime = state
        .runtimes
        .get(runtime_kind)
        .await
        .ok_or_else(|| FunctionsError::UnsupportedRuntime(runtime_kind.to_string()))?;

    // Acquire concurrency permit
    let semaphore = get_or_create_semaphore(&state, deployment_id, manifest.concurrency.max_concurrency as i32).await;
    let permit = match semaphore.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return Err(FunctionsError::TooManyRequests {
                retry_after: 1,
            });
        }
    };

    // Build deployment handle from DB record and manifest
    let limits = crate::runtime::Limits::from(&manifest);
    let handle = crate::runtime::DeploymentHandle {
        deployment_id,
        function_name: function.name.clone(),
        runtime: runtime_kind,
        version: deployment.version,
        limits,
        max_concurrency: manifest.concurrency.max_concurrency,
        runtime_ref: deployment.runtime_ref.clone(),
    };

    // Build incoming request
    let incoming = build_incoming_request(
        method,
        subpath,
        &headers,
        &ctx,
        request,
        &handle,
        &state.config.data_key,
    )?;

    // Invoke with timeout
    let timeout_duration = Duration::from_millis(handle.limits.timeout_ms);
    let result = match timeout(timeout_duration, runtime.invoke(&handle, incoming)).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            drop(permit);
            return Err(e);
        }
        Err(_) => {
            drop(permit);
            return Err(FunctionsError::InvocationTimeout);
        }
    };

    // Build response
    let response = build_response(
        result,
        &ctx.request_id,
        &function.name,
        start.elapsed().as_millis() as u64,
        permit,
    );

    Ok(response)
}

/// Path parameters for invoke routes.
#[derive(Debug, serde::Deserialize)]
pub struct InvokeParams {
    /// Function name.
    pub name: String,
    /// Optional subpath.
    #[serde(default)]
    pub rest: Option<String>,
}

/// Get or create a concurrency semaphore for a deployment.
async fn get_or_create_semaphore(
    _state: &FunctionsState,
    _deployment_id: uuid::Uuid,
    max_concurrency: i32,
) -> Arc<Semaphore> {
    // TODO: Store semaphores in a deployment-keyed cache
    // For now, create a new semaphore each time (will be improved in PR 11)
    Arc::new(Semaphore::new(max_concurrency.max(1) as usize))
}

/// Build the incoming request for the runtime.
fn build_incoming_request(
    method: Method,
    subpath: &str,
    headers: &HeaderMap,
    ctx: &FunctionCtx,
    request: Request<Body>,
    handle: &crate::runtime::DeploymentHandle,
    _data_key: &str,
) -> Result<IncomingRequest, FunctionsError> {
    let (parts, body) = request.into_parts();

    // Build path with leading slash
    let path = if subpath.is_empty() {
        "/".to_string()
    } else if subpath.starts_with('/') {
        subpath.to_string()
    } else {
        format!("/{}", subpath)
    };

    // Extract query string
    let query = parts.uri.query().map(|s| s.to_string());

    // Build headers, stripping Authorization unless forward_authorization is set
    let mut out_headers = HashMap::new();
    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();

        // Skip hop-by-hop headers
        if matches!(
            name_str.as_str(),
            "connection" | "keep-alive" | "proxy-authenticate" | "proxy-authorization" | "te"
                | "trailers" | "transfer-encoding" | "upgrade"
        ) {
            continue;
        }

        // Skip Authorization header (will be replaced with X-Reactor-Auth)
        // TODO: Check manifest.forward_authorization
        if name_str == "authorization" {
            continue;
        }

        if let Ok(v) = value.to_str() {
            out_headers.insert(name_str, v.to_string());
        }
    }

    // Add X-Reactor-Auth header with auth context
    let reactor_auth = ReactorAuthHeader {
        user_id: ctx.user_id().map(|id| id.to_string()),
        org_id: ctx.active_org().to_string(),
        permissions: ctx.auth.permissions.clone(),
    };
    if let Ok(auth_json) = serde_json::to_string(&reactor_auth) {
        out_headers.insert("x-reactor-auth".to_string(), auth_json);
    }

    // Add request ID
    out_headers.insert("x-request-id".to_string(), ctx.request_id.clone());

    // Build content length
    let content_length = parts
        .headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    // Validate body size
    if let Some(len) = content_length {
        if len > handle.limits.max_body_in_bytes {
            return Err(FunctionsError::RequestBodyTooLarge {
                size: len,
                max: handle.limits.max_body_in_bytes,
            });
        }
    }

    // Wrap body with size limit
    // TODO: Implement streaming body size limit enforcement
    let _max_body_in = handle.limits.max_body_in_bytes;
    let body_stream = body.into_data_stream().map(move |chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    });

    let mut incoming = IncomingRequest::new(method.to_string(), path);
    if let Some(q) = query {
        incoming = incoming.with_query(q);
    }
    for (k, v) in out_headers {
        incoming = incoming.with_header(k, v);
    }
    incoming = incoming.with_body(body_stream, content_length);

    Ok(incoming)
}

/// Build the HTTP response from the invoke result.
fn build_response(
    result: InvokeResult,
    request_id: &str,
    function_name: &str,
    duration_ms: u64,
    _permit: OwnedSemaphorePermit, // Hold permit until response is sent
) -> Response {
    let InvokeResult {
        response,
        cold_start,
        duration_ms: _runtime_duration,
    } = result;

    // Build response with platform headers
    let mut builder = Response::builder().status(StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK));

    // Add response headers
    for (name, value) in &response.headers {
        builder = builder.header(name, value);
    }

    // Add platform headers
    builder = builder
        .header("x-request-id", request_id)
        .header("x-reactor-function", function_name)
        .header("x-reactor-cold-start", if cold_start { "true" } else { "false" })
        .header("x-reactor-duration-ms", duration_ms.to_string());

    // Build body
    let body = Body::from_stream(response.body);

    builder.body(body).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("failed to build response"))
            .unwrap()
    })
}
