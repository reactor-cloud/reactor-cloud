//! Function runtime abstractions and adapters.
//!
//! The `FunctionRuntime` trait defines the interface for executing functions
//! across different backends (wasm, bun, lambda).

#[cfg(feature = "runtime-bun")]
mod bun;
#[cfg(feature = "runtime-lambda")]
mod lambda;
#[cfg(feature = "runtime-wasm")]
mod wasm;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::bundle::{Manifest, RuntimeKind};
use crate::error::FunctionsError;
use crate::store::DeploymentId;

#[cfg(feature = "runtime-bun")]
pub use bun::{BunRuntime, BunRuntimeConfig};
#[cfg(feature = "runtime-lambda")]
pub use lambda::{LambdaRuntime, LambdaRuntimeConfig};
#[cfg(feature = "runtime-wasm")]
pub use wasm::{WasmRuntime, WasmRuntimeConfig};

/// Incoming HTTP request to a function.
pub struct IncomingRequest {
    /// HTTP method.
    pub method: String,
    /// Request URI path (relative to the function root).
    pub path: String,
    /// Query string (without leading ?).
    pub query: Option<String>,
    /// HTTP headers.
    pub headers: HashMap<String, String>,
    /// Request body as a byte stream.
    pub body: Option<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>>,
    /// Content-Length if known.
    pub content_length: Option<u64>,
}

impl std::fmt::Debug for IncomingRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IncomingRequest")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query", &self.query)
            .field("headers", &self.headers)
            .field("body", &self.body.is_some())
            .field("content_length", &self.content_length)
            .finish()
    }
}

impl IncomingRequest {
    /// Create a new incoming request.
    pub fn new(method: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            path: path.into(),
            query: None,
            headers: HashMap::new(),
            body: None,
            content_length: None,
        }
    }

    /// Set the query string.
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Add a header.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Set the body.
    pub fn with_body(
        mut self,
        body: impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
        content_length: Option<u64>,
    ) -> Self {
        self.body = Some(Box::pin(body));
        self.content_length = content_length;
        self
    }
}

/// Outgoing HTTP response from a function.
pub struct OutgoingResponse {
    /// HTTP status code.
    pub status: u16,
    /// HTTP headers.
    pub headers: HashMap<String, String>,
    /// Response body as a byte stream.
    pub body: Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>,
}

impl std::fmt::Debug for OutgoingResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutgoingResponse")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("body", &"<stream>")
            .finish()
    }
}

impl OutgoingResponse {
    /// Create a new outgoing response.
    pub fn new(
        status: u16,
        body: impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
    ) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: Box::pin(body),
        }
    }

    /// Add a header.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }
}

/// Resource limits for a function invocation.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Memory limit in bytes.
    pub memory_bytes: u64,
    /// Maximum request body size in bytes.
    pub max_body_in_bytes: u64,
    /// Maximum response body size in bytes.
    pub max_body_out_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            memory_bytes: 256 * 1024 * 1024,
            max_body_in_bytes: 5 * 1024 * 1024,
            max_body_out_bytes: 6 * 1024 * 1024,
        }
    }
}

impl From<&Manifest> for Limits {
    fn from(manifest: &Manifest) -> Self {
        Self {
            timeout_ms: manifest.limits.timeout_ms,
            memory_bytes: (manifest.limits.memory_mb as u64) * 1024 * 1024,
            max_body_in_bytes: (manifest.limits.max_body_in_mb as u64) * 1024 * 1024,
            max_body_out_bytes: (manifest.limits.max_body_out_mb as u64) * 1024 * 1024,
        }
    }
}

/// Handle to a deployed function instance.
#[derive(Debug, Clone)]
pub struct DeploymentHandle {
    /// Deployment ID.
    pub deployment_id: DeploymentId,
    /// Function name.
    pub function_name: String,
    /// Runtime kind.
    pub runtime: RuntimeKind,
    /// Version number.
    pub version: i64,
    /// Resource limits.
    pub limits: Limits,
    /// Concurrency settings.
    pub max_concurrency: u32,
    /// Runtime-specific reference (e.g., Lambda ARN, precompiled module path).
    pub runtime_ref: Option<String>,
}

/// Invocation result with metadata.
#[derive(Debug)]
pub struct InvokeResult {
    /// The HTTP response.
    pub response: OutgoingResponse,
    /// Whether this was a cold start.
    pub cold_start: bool,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Function runtime trait.
///
/// Implementations provide the ability to deploy and invoke functions
/// on different backends (wasm, bun, lambda).
#[async_trait]
pub trait FunctionRuntime: Send + Sync {
    /// Get the runtime kind.
    fn kind(&self) -> RuntimeKind;

    /// Deploy a function bundle to this runtime.
    ///
    /// Returns a handle to the deployed instance, or an error if deployment fails.
    async fn deploy(
        &self,
        deployment_id: DeploymentId,
        function_name: &str,
        manifest: &Manifest,
        bundle_path: &std::path::Path,
    ) -> Result<DeploymentHandle, FunctionsError>;

    /// Invoke a deployed function.
    ///
    /// The request is streamed to the function, and the response is streamed back.
    async fn invoke(
        &self,
        handle: &DeploymentHandle,
        request: IncomingRequest,
    ) -> Result<InvokeResult, FunctionsError>;

    /// Warm up instances for a deployment.
    ///
    /// This is a hint to the runtime to prepare instances ahead of time.
    async fn warm(&self, handle: &DeploymentHandle, count: u32) -> Result<(), FunctionsError>;

    /// Destroy a deployed function, cleaning up resources.
    async fn destroy(&self, handle: &DeploymentHandle) -> Result<(), FunctionsError>;

    /// List active deployments in this runtime.
    async fn list_active(&self) -> Result<Vec<DeploymentHandle>, FunctionsError>;
}

/// Registry of available runtimes.
pub struct RuntimeRegistry {
    runtimes: RwLock<HashMap<RuntimeKind, Arc<dyn FunctionRuntime>>>,
}

impl RuntimeRegistry {
    /// Create a new runtime registry.
    pub fn new() -> Self {
        Self {
            runtimes: RwLock::new(HashMap::new()),
        }
    }

    /// Register a runtime.
    pub async fn register(&self, runtime: Arc<dyn FunctionRuntime>) {
        let kind = runtime.kind();
        let mut runtimes = self.runtimes.write().await;
        runtimes.insert(kind, runtime);
    }

    /// Get a runtime by kind.
    pub async fn get(&self, kind: RuntimeKind) -> Option<Arc<dyn FunctionRuntime>> {
        let runtimes = self.runtimes.read().await;
        runtimes.get(&kind).cloned()
    }

    /// List registered runtime kinds.
    pub async fn list(&self) -> Vec<RuntimeKind> {
        let runtimes = self.runtimes.read().await;
        runtimes.keys().copied().collect()
    }
}

impl Default for RuntimeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
