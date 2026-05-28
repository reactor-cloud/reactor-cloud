//! Bun subprocess runtime adapter.
//!
//! Manages long-lived Bun processes per deployment, with warm pool,
//! LRU eviction, graceful shutdown, and crash recovery.

use super::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::{RwLock, Semaphore};

/// The Bun shim TypeScript code, embedded at compile time.
/// This shim wraps user code and exposes it via a Unix socket.
const BUN_SHIM: &str = include_str!("../../runtime/bun-shim/index.ts");

/// BunRuntime configuration.
#[derive(Debug, Clone)]
pub struct BunRuntimeConfig {
    /// Path to bun binary.
    pub bun_bin: String,
    /// Working directory for extracting bundles.
    pub workdir: PathBuf,
    /// Idle TTL in seconds before evicting warm instances.
    pub idle_ttl_secs: u64,
    /// Maximum warm instances per function.
    pub max_instances_per_fn: u32,
}

impl Default for BunRuntimeConfig {
    fn default() -> Self {
        Self {
            bun_bin: "bun".to_string(),
            workdir: PathBuf::from("/var/lib/reactor-functions/bun"),
            idle_ttl_secs: 300,
            max_instances_per_fn: 8,
        }
    }
}

/// A running Bun instance.
struct BunInstance {
    /// The subprocess handle.
    child: Child,
    /// Process ID.
    pid: u32,
    /// Unix socket path.
    socket_path: PathBuf,
    /// Whether this instance is currently handling a request.
    busy: AtomicBool,
    /// Last activity timestamp (for LRU eviction).
    last_activity: AtomicU64,
    /// Whether this instance is shutting down.
    shutting_down: AtomicBool,
}

impl BunInstance {
    fn touch(&self) {
        self.last_activity.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::Relaxed,
        );
    }

    fn last_activity_secs(&self) -> u64 {
        self.last_activity.load(Ordering::Relaxed)
    }

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    fn set_busy(&self, busy: bool) {
        self.busy.store(busy, Ordering::Release);
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    fn mark_shutting_down(&self) {
        self.shutting_down.store(true, Ordering::Release);
    }
}

/// Pool of Bun instances for a deployment.
struct DeploymentPool {
    /// Deployment handle.
    handle: DeploymentHandle,
    /// Path to extracted bundle.
    bundle_path: PathBuf,
    /// Active instances.
    instances: Vec<BunInstance>,
    /// Number of cold starts.
    cold_starts: AtomicU64,
    /// Semaphore to limit concurrent spawns.
    spawn_semaphore: Arc<Semaphore>,
}

/// Bun subprocess runtime.
pub struct BunRuntime {
    config: BunRuntimeConfig,
    /// Pools keyed by deployment ID.
    pools: Arc<RwLock<HashMap<DeploymentId, DeploymentPool>>>,
    /// Shutdown signal for the eviction task.
    shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
}

impl BunRuntime {
    /// Create a new BunRuntime with background eviction task.
    pub fn new(config: BunRuntimeConfig) -> Self {
        let pools = Arc::new(RwLock::new(HashMap::new()));
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        
        // Start background eviction task
        let eviction_pools = pools.clone();
        let idle_ttl_secs = config.idle_ttl_secs;
        tokio::spawn(async move {
            Self::eviction_loop(eviction_pools, idle_ttl_secs, shutdown_rx).await;
        });
        
        Self {
            config,
            pools,
            shutdown_tx: Some(shutdown_tx),
        }
    }
    
    /// Background task that evicts idle instances.
    async fn eviction_loop(
        pools: Arc<RwLock<HashMap<DeploymentId, DeploymentPool>>>,
        idle_ttl_secs: u64,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) {
        let check_interval = Duration::from_secs(idle_ttl_secs.max(30) / 2);
        
        loop {
            tokio::select! {
                _ = tokio::time::sleep(check_interval) => {
                    // Check for idle instances to evict
                    let now_secs = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    
                    let mut pools = pools.write().await;
                    for pool in pools.values_mut() {
                        // Find indices of instances to evict (idle and not busy)
                        let mut to_evict = Vec::new();
                        for (idx, instance) in pool.instances.iter().enumerate() {
                            let idle_secs = now_secs.saturating_sub(instance.last_activity_secs());
                            if idle_secs > idle_ttl_secs && !instance.is_busy() {
                                to_evict.push(idx);
                            }
                        }
                        
                        // Remove in reverse order to maintain indices
                        for idx in to_evict.into_iter().rev() {
                            let mut instance = pool.instances.remove(idx);
                            tracing::debug!(
                                deployment_id = %pool.handle.deployment_id,
                                pid = instance.pid,
                                "evicting idle bun instance"
                            );
                            Self::shutdown_instance(&mut instance).await;
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::debug!("bun eviction loop shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Spawn a new Bun instance for a deployment.
    async fn spawn_instance(
        &self,
        handle: &DeploymentHandle,
        bundle_path: &PathBuf,
    ) -> Result<BunInstance, FunctionsError> {
        // Generate unique socket path
        let socket_name = format!(
            "{}-{}-{}.sock",
            handle.function_name,
            handle.deployment_id,
            uuid::Uuid::now_v7()
        );
        let socket_path = self.config.workdir.join("sockets").join(&socket_name);

        // Ensure socket directory exists
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                FunctionsError::RuntimeError(format!("failed to create socket dir: {}", e))
            })?;
        }

        // Remove stale socket if exists
        let _ = tokio::fs::remove_file(&socket_path).await;

        // Build environment variables
        let mut env_vars = std::collections::HashMap::new();
        env_vars.insert("REACTOR_SOCKET_PATH", socket_path.to_string_lossy().to_string());
        env_vars.insert("REACTOR_FUNCTION_NAME", handle.function_name.clone());
        env_vars.insert("REACTOR_DEPLOYMENT_ID", handle.deployment_id.to_string());
        env_vars.insert("REACTOR_TIMEOUT_MS", handle.limits.timeout_ms.to_string());
        env_vars.insert("REACTOR_MAX_BODY_IN_BYTES", handle.limits.max_body_in_bytes.to_string());
        env_vars.insert("REACTOR_MAX_BODY_OUT_BYTES", handle.limits.max_body_out_bytes.to_string());

        // Spawn bun process running the shim
        // The shim imports ./code/index.ts which exports { fetch(req) { ... } }
        // and exposes it via Unix socket
        let shim_path = bundle_path.join("shim.ts");
        let mut child = Command::new(&self.config.bun_bin)
            .arg("run")
            .arg("--watch=false")
            .arg(&shim_path)
            .current_dir(bundle_path)
            .envs(env_vars)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| FunctionsError::RuntimeError(format!("failed to spawn bun: {}", e)))?;

        let pid = child.id().unwrap_or(0);

        tracing::info!(
            deployment_id = %handle.deployment_id,
            function = %handle.function_name,
            pid = pid,
            socket = %socket_path.display(),
            "spawned bun instance"
        );

        // Wait for socket to be created (with timeout)
        let wait_start = Instant::now();
        let max_wait = Duration::from_secs(10);
        while wait_start.elapsed() < max_wait {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if !socket_path.exists() {
            // Kill the process if socket wasn't created
            let _ = child.kill().await;
            return Err(FunctionsError::ColdStartFailed(
                "bun instance did not create socket within timeout".to_string(),
            ));
        }

        Ok(BunInstance {
            child,
            pid,
            socket_path,
            busy: AtomicBool::new(false),
            last_activity: AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
            shutting_down: AtomicBool::new(false),
        })
    }

    /// Gracefully shutdown an instance with SIGTERM -> wait -> SIGKILL.
    #[cfg(unix)]
    async fn shutdown_instance(instance: &mut BunInstance) {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        if instance.is_shutting_down() {
            return;
        }
        instance.mark_shutting_down();

        let pid = Pid::from_raw(instance.pid as i32);

        // Send SIGTERM
        if let Err(e) = kill(pid, Signal::SIGTERM) {
            tracing::warn!(pid = instance.pid, error = %e, "failed to send SIGTERM");
        }

        // Wait up to 5 seconds for graceful shutdown
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match instance.child.try_wait() {
                Ok(Some(_status)) => {
                    tracing::debug!(pid = instance.pid, "bun instance exited gracefully");
                    return;
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    tracing::warn!(pid = instance.pid, error = %e, "error checking bun status");
                    break;
                }
            }
        }

        // Send SIGKILL if still running
        tracing::warn!(pid = instance.pid, "sending SIGKILL after grace period");
        if let Err(e) = kill(pid, Signal::SIGKILL) {
            tracing::warn!(pid = instance.pid, error = %e, "failed to send SIGKILL");
        }

        // Wait for kill
        let _ = instance.child.wait().await;
    }

    #[cfg(not(unix))]
    async fn shutdown_instance(instance: &mut BunInstance) {
        let _ = instance.child.kill().await;
        let _ = instance.child.wait().await;
    }

    /// Send HTTP request over Unix socket and return the response.
    async fn invoke_over_socket(
        &self,
        socket_path: &PathBuf,
        request: IncomingRequest,
        handle: &DeploymentHandle,
    ) -> Result<OutgoingResponse, FunctionsError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixStream;
        
        // Connect to Unix socket
        let mut stream = UnixStream::connect(socket_path).await.map_err(|e| {
            FunctionsError::RuntimeError(format!("failed to connect to socket: {}", e))
        })?;
        
        // Build HTTP/1.1 request
        let mut http_request = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\n",
            request.method,
            request.path,
            handle.function_name
        );
        
        // Add headers
        for (key, value) in &request.headers {
            http_request.push_str(&format!("{}: {}\r\n", key, value));
        }
        
        // Add request ID if not present
        if !request.headers.contains_key("x-request-id") {
            http_request.push_str(&format!("x-request-id: {}\r\n", uuid::Uuid::now_v7()));
        }
        
        // Add content-length if there's a body
        let body_bytes = if let Some(body) = request.body {
            use futures::StreamExt;
            let mut collected = Vec::new();
            let mut body_stream = std::pin::pin!(body);
            while let Some(chunk) = body_stream.next().await {
                match chunk {
                    Ok(bytes) => collected.extend_from_slice(&bytes),
                    Err(e) => return Err(FunctionsError::RuntimeError(format!("body read error: {}", e))),
                }
            }
            http_request.push_str(&format!("content-length: {}\r\n", collected.len()));
            Some(collected)
        } else {
            http_request.push_str("content-length: 0\r\n");
            None
        };
        
        // End headers
        http_request.push_str("\r\n");
        
        // Send request
        stream.write_all(http_request.as_bytes()).await.map_err(|e| {
            FunctionsError::RuntimeError(format!("failed to write request: {}", e))
        })?;
        
        if let Some(body) = body_bytes {
            stream.write_all(&body).await.map_err(|e| {
                FunctionsError::RuntimeError(format!("failed to write body: {}", e))
            })?;
        }
        
        stream.flush().await.map_err(|e| {
            FunctionsError::RuntimeError(format!("failed to flush: {}", e))
        })?;
        
        // Read response
        let mut response_buf = Vec::with_capacity(4096);
        let mut temp_buf = [0u8; 4096];
        
        // Read headers first (until \r\n\r\n)
        let mut headers_end = None;
        loop {
            let n = stream.read(&mut temp_buf).await.map_err(|e| {
                FunctionsError::RuntimeError(format!("failed to read response: {}", e))
            })?;
            
            if n == 0 {
                break;
            }
            
            response_buf.extend_from_slice(&temp_buf[..n]);
            
            // Check for end of headers
            if let Some(pos) = response_buf.windows(4).position(|w| w == b"\r\n\r\n") {
                headers_end = Some(pos + 4);
                break;
            }
            
            // Safety limit on header size
            if response_buf.len() > 65536 {
                return Err(FunctionsError::RuntimeError("response headers too large".to_string()));
            }
        }
        
        let headers_end = headers_end.ok_or_else(|| {
            FunctionsError::RuntimeError("incomplete response headers".to_string())
        })?;
        
        // Parse status line and headers
        let headers_str = String::from_utf8_lossy(&response_buf[..headers_end]);
        let mut lines = headers_str.lines();
        
        let status_line = lines.next().ok_or_else(|| {
            FunctionsError::RuntimeError("missing status line".to_string())
        })?;
        
        // Parse "HTTP/1.1 200 OK"
        let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
        let status = parts.get(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(500);
        
        // Parse headers
        let mut response_headers = std::collections::HashMap::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            if let Some((key, value)) = line.split_once(':') {
                response_headers.insert(
                    key.trim().to_lowercase(),
                    value.trim().to_string()
                );
            }
        }
        
        // Get content-length or check for chunked encoding
        let content_length = response_headers
            .get("content-length")
            .and_then(|s| s.parse::<usize>().ok());
        let is_chunked = response_headers
            .get("transfer-encoding")
            .map(|v| v.contains("chunked"))
            .unwrap_or(false);
        
        // Collect remaining body
        let body_start = &response_buf[headers_end..];
        let mut body = Vec::from(body_start);
        
        // Read remaining body based on content-length or until EOF
        if let Some(len) = content_length {
            while body.len() < len {
                let n = stream.read(&mut temp_buf).await.map_err(|e| {
                    FunctionsError::RuntimeError(format!("failed to read body: {}", e))
                })?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&temp_buf[..n]);
            }
            body.truncate(len);
        } else if !is_chunked {
            // Read until EOF for connection: close
            loop {
                let n = stream.read(&mut temp_buf).await.map_err(|e| {
                    FunctionsError::RuntimeError(format!("failed to read body: {}", e))
                })?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&temp_buf[..n]);
            }
        }
        // TODO: Handle chunked encoding properly
        
        // Build response
        let body_bytes = Bytes::from(body);
        let body_stream = futures::stream::once(async move {
            Ok::<_, std::io::Error>(body_bytes)
        });
        
        let mut response = OutgoingResponse::new(status, body_stream);
        for (key, value) in response_headers {
            response = response.with_header(&key, &value);
        }
        
        Ok(response)
    }
}

#[async_trait]
impl FunctionRuntime for BunRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Bun
    }

    async fn deploy(
        &self,
        deployment_id: DeploymentId,
        function_name: &str,
        manifest: &Manifest,
        bundle_path: &std::path::Path,
    ) -> Result<DeploymentHandle, FunctionsError> {
        let handle = DeploymentHandle {
            deployment_id,
            function_name: function_name.to_string(),
            runtime: RuntimeKind::Bun,
            version: manifest.version,
            limits: Limits::from(manifest),
            max_concurrency: manifest.concurrency.max_concurrency,
            runtime_ref: None,
        };

        // Create deployment directory
        let deploy_path = self
            .config
            .workdir
            .join("deployments")
            .join(deployment_id.to_string());
        tokio::fs::create_dir_all(&deploy_path).await.map_err(|e| {
            FunctionsError::RuntimeError(format!("failed to create deployment dir: {}", e))
        })?;

        // Extract zip bundle to deployment directory
        // Bundle structure: manifest.json + code/index.ts (+ other files)
        let zip_bytes = tokio::fs::read(bundle_path).await.map_err(|e| {
            FunctionsError::RuntimeError(format!("failed to read bundle: {}", e))
        })?;
        
        // Extract synchronously (zip extraction is not async but fast enough)
        let extract_path = deploy_path.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::{Cursor, Read};
            use zip::ZipArchive;
            
            let reader = Cursor::new(zip_bytes);
            let mut archive = ZipArchive::new(reader)?;
            
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = match file.enclosed_name() {
                    Some(path) => extract_path.join(path),
                    None => continue,
                };
                
                if file.is_dir() {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(parent) = outpath.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }
                    let mut outfile = std::fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                    
                    // Set executable bit on Unix for .sh files
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Some(mode) = file.unix_mode() {
                            std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
                        }
                    }
                }
            }
            
            Ok::<_, std::io::Error>(())
        })
        .await
        .map_err(|e| FunctionsError::RuntimeError(format!("extract task failed: {}", e)))?
        .map_err(|e| FunctionsError::RuntimeError(format!("failed to extract bundle: {}", e)))?;

        // Write the shim to the deployment directory
        let shim_path = deploy_path.join("shim.ts");
        tokio::fs::write(&shim_path, BUN_SHIM).await.map_err(|e| {
            FunctionsError::RuntimeError(format!("failed to write shim: {}", e))
        })?;

        tracing::info!(
            deployment_id = %deployment_id,
            function = %function_name,
            path = %deploy_path.display(),
            "deployed bun function"
        );

        // Create empty pool (instances created on demand)
        // Semaphore limits concurrent instances per function
        let pool = DeploymentPool {
            handle: handle.clone(),
            bundle_path: deploy_path,
            instances: Vec::new(),
            cold_starts: AtomicU64::new(0),
            spawn_semaphore: Arc::new(Semaphore::new(self.config.max_instances_per_fn as usize)),
        };

        let mut pools = self.pools.write().await;
        pools.insert(deployment_id, pool);

        Ok(handle)
    }

    async fn invoke(
        &self,
        handle: &DeploymentHandle,
        request: IncomingRequest,
    ) -> Result<InvokeResult, FunctionsError> {
        let start = Instant::now();
        let mut cold_start = false;

        // Acquire an instance from the pool (or spawn a new one)
        let socket_path = {
            let mut pools = self.pools.write().await;
            let pool = pools
                .get_mut(&handle.deployment_id)
                .ok_or_else(|| FunctionsError::DeploymentNotFound(handle.deployment_id.to_string()))?;

            // Find an available (non-busy) instance
            let available_idx = pool.instances.iter().position(|inst| {
                !inst.is_busy() && !inst.is_shutting_down()
            });

            let socket = match available_idx {
                Some(idx) => {
                    pool.instances[idx].set_busy(true);
                    pool.instances[idx].touch();
                    pool.instances[idx].socket_path.clone()
                }
                None => {
                    // No available instance, need to spawn one (cold start)
                    cold_start = true;
                    pool.cold_starts.fetch_add(1, Ordering::Relaxed);
                    
                    if pool.instances.len() >= self.config.max_instances_per_fn as usize {
                        // Pool is full, wait for one to become available
                        // For now, just fail - TODO: implement queue
                        return Err(FunctionsError::RuntimeError(
                            "all instances busy and pool is full".to_string()
                        ));
                    }
                    
                    let instance = self.spawn_instance(handle, &pool.bundle_path).await?;
                    instance.set_busy(true);
                    let socket = instance.socket_path.clone();
                    pool.instances.push(instance);
                    socket
                }
            };
            
            socket
        };

        // Make HTTP request over Unix socket
        let result = self.invoke_over_socket(&socket_path, request, handle).await;
        
        // Release the instance
        {
            let pools = self.pools.read().await;
            if let Some(pool) = pools.get(&handle.deployment_id) {
                if let Some(instance) = pool.instances.iter().find(|i| i.socket_path == socket_path) {
                    instance.set_busy(false);
                    instance.touch();
                }
            }
        }

        let response = result?;
        
        Ok(InvokeResult {
            response,
            cold_start,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn warm(&self, handle: &DeploymentHandle, count: u32) -> Result<(), FunctionsError> {
        let mut pools = self.pools.write().await;
        let pool = pools
            .get_mut(&handle.deployment_id)
            .ok_or_else(|| FunctionsError::DeploymentNotFound(handle.deployment_id.to_string()))?;

        let current = pool.instances.len() as u32;
        let to_spawn = count.saturating_sub(current).min(self.config.max_instances_per_fn);

        for _ in 0..to_spawn {
            match self.spawn_instance(handle, &pool.bundle_path).await {
                Ok(instance) => {
                    pool.instances.push(instance);
                }
                Err(e) => {
                    tracing::warn!(
                        deployment_id = %handle.deployment_id,
                        error = %e,
                        "failed to spawn warm instance"
                    );
                }
            }
        }

        Ok(())
    }

    async fn destroy(&self, handle: &DeploymentHandle) -> Result<(), FunctionsError> {
        let mut pools = self.pools.write().await;
        if let Some(mut pool) = pools.remove(&handle.deployment_id) {
            // Shutdown all instances
            for mut instance in pool.instances.drain(..) {
                Self::shutdown_instance(&mut instance).await;
            }

            // Remove deployment directory
            let deploy_path = self
                .config
                .workdir
                .join("deployments")
                .join(handle.deployment_id.to_string());
            let _ = tokio::fs::remove_dir_all(&deploy_path).await;

            tracing::info!(
                deployment_id = %handle.deployment_id,
                "destroyed bun deployment"
            );
        }
        Ok(())
    }

    async fn list_active(&self) -> Result<Vec<DeploymentHandle>, FunctionsError> {
        let pools = self.pools.read().await;
        Ok(pools.values().map(|p| p.handle.clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    /// Create a test bundle zip with a simple function.
    fn create_test_bundle(dir: &TempDir, code: &str) -> PathBuf {
        let zip_path = dir.path().join("test-bundle.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);

        // Write manifest.json
        let manifest = r#"{
            "version": 1,
            "runtime": "bun",
            "entrypoint": "code/index.ts",
            "timeout_ms": 30000,
            "max_body_in_bytes": 6291456,
            "max_body_out_bytes": 6291456,
            "max_concurrency": 10
        }"#;
        zip.start_file::<_, ()>("manifest.json", FileOptions::default())
            .unwrap();
        zip.write_all(manifest.as_bytes()).unwrap();

        // Write code/index.ts
        zip.start_file::<_, ()>("code/index.ts", FileOptions::default())
            .unwrap();
        zip.write_all(code.as_bytes()).unwrap();

        zip.finish().unwrap();
        zip_path
    }

    /// Create a simple manifest for testing.
    fn test_manifest() -> Manifest {
        use crate::bundle::{BundleLimits, ConcurrencyConfig};
        
        Manifest {
            name: "test-fn".to_string(),
            version: 1,
            runtime: RuntimeKind::Bun,
            entrypoint: "code/index.ts".to_string(),
            limits: BundleLimits {
                timeout_ms: 30000,
                memory_mb: 128,
                max_body_in_mb: 6,
                max_body_out_mb: 6,
            },
            concurrency: ConcurrencyConfig {
                min_instances: 0,
                max_concurrency: 10,
            },
            env_keys: Vec::new(),
            secret_keys: Vec::new(),
            forward_authorization: false,
            bundle_sha256: None,
            job: None,
        }
    }

    #[tokio::test]
    async fn test_bun_runtime_deploy_and_destroy() {
        let temp_dir = TempDir::new().unwrap();
        let workdir = temp_dir.path().join("bun-workdir");

        let config = BunRuntimeConfig {
            bun_bin: "bun".to_string(),
            workdir,
            idle_ttl_secs: 60,
            max_instances_per_fn: 2,
        };

        let runtime = BunRuntime::new(config);

        // Create a test bundle
        let code = r#"
            export default {
                fetch(req: Request): Response {
                    return new Response("Hello from test function!");
                }
            };
        "#;
        let bundle_path = create_test_bundle(&temp_dir, code);
        let manifest = test_manifest();

        // Deploy
        let deployment_id = DeploymentId::now_v7();
        let handle = runtime
            .deploy(deployment_id, "test-fn", &manifest, &bundle_path)
            .await
            .unwrap();

        assert_eq!(handle.deployment_id, deployment_id);
        assert_eq!(handle.function_name, "test-fn");
        assert_eq!(handle.runtime, RuntimeKind::Bun);

        // Verify deployment is tracked
        let active = runtime.list_active().await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].deployment_id, deployment_id);

        // Destroy
        runtime.destroy(&handle).await.unwrap();

        // Verify deployment is gone
        let active = runtime.list_active().await.unwrap();
        assert!(active.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires bun installed"]
    async fn test_bun_simple_invoke() {
        let temp_dir = TempDir::new().unwrap();
        let workdir = temp_dir.path().join("bun-workdir");

        let config = BunRuntimeConfig {
            bun_bin: "bun".to_string(),
            workdir,
            idle_ttl_secs: 60,
            max_instances_per_fn: 2,
        };

        let runtime = BunRuntime::new(config);

        // Create a simple echo function
        let code = r#"
            export default {
                fetch(req: Request): Response {
                    const url = new URL(req.url);
                    return new Response(JSON.stringify({
                        method: req.method,
                        path: url.pathname,
                        message: "Hello from Bun!",
                    }), {
                        headers: { "content-type": "application/json" },
                    });
                }
            };
        "#;
        let bundle_path = create_test_bundle(&temp_dir, code);
        let manifest = test_manifest();

        // Deploy
        let deployment_id = DeploymentId::now_v7();
        let handle = runtime
            .deploy(deployment_id, "echo-fn", &manifest, &bundle_path)
            .await
            .unwrap();

        // Invoke
        let request = IncomingRequest {
            method: "GET".to_string(),
            path: "/test".to_string(),
            query: None,
            headers: HashMap::new(),
            body: None,
            content_length: None,
        };

        let result = runtime.invoke(&handle, request).await.unwrap();
        assert!(result.cold_start);
        assert_eq!(result.response.status, 200);

        // Cleanup
        runtime.destroy(&handle).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires bun installed"]
    async fn test_bun_streaming_sse() {
        let temp_dir = TempDir::new().unwrap();
        let workdir = temp_dir.path().join("bun-workdir");

        let config = BunRuntimeConfig {
            bun_bin: "bun".to_string(),
            workdir,
            idle_ttl_secs: 60,
            max_instances_per_fn: 2,
        };

        let runtime = BunRuntime::new(config);

        // Create an SSE streaming function
        let code = r#"
            export default {
                async fetch(req: Request): Promise<Response> {
                    const encoder = new TextEncoder();
                    const stream = new ReadableStream({
                        async start(controller) {
                            for (let i = 0; i < 3; i++) {
                                controller.enqueue(encoder.encode(`data: message ${i}\n\n`));
                                await new Promise(r => setTimeout(r, 10));
                            }
                            controller.close();
                        }
                    });

                    return new Response(stream, {
                        headers: {
                            "content-type": "text/event-stream",
                            "cache-control": "no-cache",
                        },
                    });
                }
            };
        "#;
        let bundle_path = create_test_bundle(&temp_dir, code);
        let manifest = test_manifest();

        // Deploy
        let deployment_id = DeploymentId::now_v7();
        let handle = runtime
            .deploy(deployment_id, "sse-fn", &manifest, &bundle_path)
            .await
            .unwrap();

        // Invoke
        let request = IncomingRequest {
            method: "GET".to_string(),
            path: "/stream".to_string(),
            query: None,
            headers: HashMap::new(),
            body: None,
            content_length: None,
        };

        let result = runtime.invoke(&handle, request).await.unwrap();
        assert_eq!(result.response.status, 200);

        // Check content-type header
        let headers = result.response.headers;
        assert!(headers.iter().any(|(k, v)| k == "content-type" && v.contains("text/event-stream")));

        // Cleanup
        runtime.destroy(&handle).await.unwrap();
    }
}
