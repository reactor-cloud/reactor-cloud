//! WebAssembly runtime adapter using wasmtime + WASI Preview 2.

use super::*;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;

/// WasmRuntime configuration.
#[derive(Debug, Clone)]
pub struct WasmRuntimeConfig {
    /// Directory for storing compiled modules.
    pub cache_dir: PathBuf,
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from("/var/lib/reactor-functions/wasm-cache"),
        }
    }
}

/// WebAssembly function runtime using wasmtime.
pub struct WasmRuntime {
    config: WasmRuntimeConfig,
    /// Cache of precompiled modules by deployment ID.
    modules: RwLock<HashMap<DeploymentId, CompiledModule>>,
}

/// A precompiled WASM module.
struct CompiledModule {
    /// Path to the compiled module.
    compiled_path: PathBuf,
    /// Deployment handle.
    handle: DeploymentHandle,
}

impl WasmRuntime {
    /// Create a new WasmRuntime.
    pub fn new(config: WasmRuntimeConfig) -> Self {
        Self {
            config,
            modules: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl FunctionRuntime for WasmRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Wasm
    }

    async fn deploy(
        &self,
        deployment_id: DeploymentId,
        function_name: &str,
        manifest: &Manifest,
        bundle_path: &std::path::Path,
    ) -> Result<DeploymentHandle, FunctionsError> {
        // TODO: Implement WASM module compilation
        // 1. Extract the .wasm file from the bundle
        // 2. Precompile it using wasmtime
        // 3. Store the compiled module
        // 4. Return the deployment handle

        let handle = DeploymentHandle {
            deployment_id,
            function_name: function_name.to_string(),
            runtime: RuntimeKind::Wasm,
            version: manifest.version,
            limits: Limits::from(manifest),
            max_concurrency: manifest.concurrency.max_concurrency,
            runtime_ref: None,
        };

        tracing::info!(
            deployment_id = %deployment_id,
            function = %function_name,
            version = manifest.version,
            bundle_path = %bundle_path.display(),
            "TODO: deploy WASM module"
        );

        // Store in cache
        let mut modules = self.modules.write().await;
        modules.insert(
            deployment_id,
            CompiledModule {
                compiled_path: bundle_path.to_path_buf(),
                handle: handle.clone(),
            },
        );

        Ok(handle)
    }

    async fn invoke(
        &self,
        handle: &DeploymentHandle,
        request: IncomingRequest,
    ) -> Result<InvokeResult, FunctionsError> {
        // TODO: Implement WASM invocation
        // 1. Look up the compiled module
        // 2. Create a new instance (or reuse from pool)
        // 3. Set up WASI HTTP handlers
        // 4. Call the exported handler
        // 5. Stream the response back

        let start = std::time::Instant::now();

        tracing::debug!(
            deployment_id = %handle.deployment_id,
            method = %request.method,
            path = %request.path,
            "TODO: invoke WASM function"
        );

        // For now, return a placeholder response
        let body = futures::stream::once(async {
            Ok::<_, std::io::Error>(Bytes::from(
                r#"{"error": "WASM runtime not yet implemented"}"#,
            ))
        });

        let response = OutgoingResponse::new(501, body)
            .with_header("content-type", "application/json");

        Ok(InvokeResult {
            response,
            cold_start: true,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn warm(&self, handle: &DeploymentHandle, count: u32) -> Result<(), FunctionsError> {
        // WASM modules are always "warm" - instantiation is very fast
        // This is a no-op for the WASM runtime
        tracing::debug!(
            deployment_id = %handle.deployment_id,
            count = count,
            "warm hint ignored for WASM runtime"
        );
        Ok(())
    }

    async fn destroy(&self, handle: &DeploymentHandle) -> Result<(), FunctionsError> {
        let mut modules = self.modules.write().await;
        if modules.remove(&handle.deployment_id).is_some() {
            tracing::info!(
                deployment_id = %handle.deployment_id,
                "destroyed WASM deployment"
            );
        }
        Ok(())
    }

    async fn list_active(&self) -> Result<Vec<DeploymentHandle>, FunctionsError> {
        let modules = self.modules.read().await;
        Ok(modules.values().map(|m| m.handle.clone()).collect())
    }
}
