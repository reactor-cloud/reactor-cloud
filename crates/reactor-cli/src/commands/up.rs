//! `reactor up` — start a detached local development server.

use crate::cli::{Cli, UpArgs};
use crate::context::{save_context_config, AuthConfig, ContextConfig, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// State file for tracking detached server processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerState {
    pub pid: u32,
    pub started_at: DateTime<Utc>,
    pub bind_addr: String,
    pub database_url: String,
    pub project_root: PathBuf,
    pub context_name: String,
}

impl ServerState {
    pub fn state_dir(context: &str) -> CliResult<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| CliError::ConfigError("Cannot find home directory".into()))?;
        let dir = home.join(".reactor").join(context);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn state_file(context: &str) -> CliResult<PathBuf> {
        Ok(Self::state_dir(context)?.join("state.json"))
    }

    pub fn load(context: &str) -> CliResult<Option<Self>> {
        let path = Self::state_file(context)?;
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    pub fn save(&self) -> CliResult<()> {
        let path = Self::state_file(&self.context_name)?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn remove(context: &str) -> CliResult<()> {
        let path = Self::state_file(context)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Check if the process is still running.
    pub fn is_running(&self) -> bool {
        #[cfg(unix)]
        {
            // Try sending signal 0 to check if process exists
            let result = Command::new("kill")
                .args(["-0", &self.pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            result.map(|s| s.success()).unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            // On Windows, we could use tasklist but for simplicity, assume running
            true
        }
    }
}

pub async fn run(cli: &Cli, args: &UpArgs, output: &Output) -> CliResult<()> {
    #[cfg(not(feature = "dev"))]
    {
        let _ = (cli, args, output);
        return Err(CliError::FeatureDisabled(
            "The 'dev' feature is not enabled. Rebuild with `cargo build --features dev`.".into(),
        ));
    }

    #[cfg(feature = "dev")]
    {
        run_up(cli, args, output).await
    }
}

#[cfg(feature = "dev")]
async fn run_up(cli: &Cli, args: &UpArgs, output: &Output) -> CliResult<()> {
    // Resolve project
    let cwd = std::env::current_dir()?;
    let project = Project::try_resolve(&cwd, cli.manifest.as_deref()).ok_or_else(|| {
        CliError::ConfigError("No reactor.toml found. Run 'reactor init' first.".into())
    })?;

    let context_name = args.context.clone().unwrap_or_else(|| "local".to_string());

    // Check if already running
    if let Some(state) = ServerState::load(&context_name)? {
        if state.is_running() {
            if args.force {
                output.info(&format!("Stopping existing server (PID: {})...", state.pid));
                stop_process(state.pid)?;
                ServerState::remove(&context_name)?;
            } else {
                return Err(CliError::ConfigError(format!(
                    "Server already running on {} (PID: {}). Use --force to restart or 'reactor down' to stop.",
                    state.bind_addr, state.pid
                )));
            }
        } else {
            // Process is dead, clean up stale state
            output.info("Cleaning up stale server state...");
            ServerState::remove(&context_name)?;
        }
    }

    // Build bind address
    let bind_addr = format!("{}:{}", args.host, args.port);

    // Determine database URL
    let database_url = if args.ephemeral {
        output.info("Starting ephemeral Docker Postgres container...");
        start_ephemeral_postgres_detached(&context_name, args.port + 1000).await?
    } else if let Some(db) = &args.db {
        db.clone()
    } else if let Ok(url) = std::env::var("DATABASE_URL") {
        url
    } else {
        output.info("Starting ephemeral Docker Postgres container...");
        start_ephemeral_postgres_detached(&context_name, args.port + 1000).await?
    };

    // Start the server as a detached process
    output.info(&format!("Starting Reactor server on http://{}", bind_addr));

    let reactor_binary = std::env::current_exe()?;
    let log_dir = ServerState::state_dir(&context_name)?;
    let stdout_log = log_dir.join("stdout.log");
    let stderr_log = log_dir.join("stderr.log");

    let stdout_file = fs::File::create(&stdout_log)?;
    let stderr_file = fs::File::create(&stderr_log)?;

    // Build dev command arguments
    let mut cmd_args = vec![
        "dev".to_string(),
        "--host".to_string(),
        args.host.clone(),
        "--port".to_string(),
        args.port.to_string(),
        "--db".to_string(),
        database_url.clone(),
    ];

    if let Some(token) = &args.admin_token {
        cmd_args.push("--admin-token".to_string());
        cmd_args.push(token.clone());
    }

    if let Some(manifest) = &cli.manifest {
        cmd_args.push("--manifest".to_string());
        cmd_args.push(manifest.to_string_lossy().to_string());
    }

    let child = Command::new(&reactor_binary)
        .args(&cmd_args)
        .current_dir(&project.root)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|e| CliError::ConfigError(format!("Failed to spawn server process: {}", e)))?;

    let pid = child.id();

    // Save state
    let state = ServerState {
        pid,
        started_at: Utc::now(),
        bind_addr: bind_addr.clone(),
        database_url,
        project_root: project.root.clone(),
        context_name: context_name.clone(),
    };
    state.save()?;

    // Update context
    create_or_update_local_context(&context_name, &bind_addr, args.admin_token.as_deref())?;

    // Wait briefly and verify it started
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    if state.is_running() {
        if output.format().is_json() {
            output.success(&serde_json::json!({
                "pid": pid,
                "bind_addr": bind_addr,
                "context": context_name,
                "logs": {
                    "stdout": stdout_log,
                    "stderr": stderr_log
                }
            }))?;
        } else {
            output.success_message(&format!(
                "Server started on http://{} (PID: {})",
                bind_addr, pid
            ))?;
            output.info(&format!("Logs: {}", log_dir.display()));
            output.info(&format!("Stop with: reactor down --context {}", context_name));
        }
    } else {
        ServerState::remove(&context_name)?;
        return Err(CliError::ServerError(
            "Server process exited immediately. Check logs for details.".into(),
        ));
    }

    Ok(())
}

fn create_or_update_local_context(
    name: &str,
    bind_addr: &str,
    admin_token: Option<&str>,
) -> CliResult<()> {
    let mut config = GlobalConfig::load()?;

    let endpoint = format!("http://{}", bind_addr);
    let auth = if let Some(_token) = admin_token {
        AuthConfig::TokenEnv { env: format!("REACTOR_LOCAL_TOKEN_{}", name.to_uppercase()) }
    } else {
        AuthConfig::None
    };

    let context = ContextConfig {
        endpoint,
        auth,
        org: None,
    };

    config.contexts.insert(name.to_string(), context);
    config.active_context = Some(name.to_string());
    config.save()?;
    save_context_config(&config, name)?;

    Ok(())
}

fn stop_process(pid: u32) -> CliResult<()> {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        // Give it time to shut down gracefully
        std::thread::sleep(std::time::Duration::from_secs(2));
        // Force kill if still running
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
    Ok(())
}

#[cfg(feature = "dev")]
async fn start_ephemeral_postgres_detached(context: &str, port: u16) -> CliResult<String> {
    use tokio::process::Command as TokioCommand;

    let container_name = format!("reactor-dev-postgres-{}", context);

    // Check if container already exists
    let check = TokioCommand::new("docker")
        .args(["ps", "-q", "-f", &format!("name={}", container_name)])
        .output()
        .await
        .map_err(|e| CliError::ConfigError(format!("Docker not available: {}", e)))?;

    if !check.stdout.is_empty() {
        return Ok(format!(
            "postgres://reactor:reactor@localhost:{}/reactor",
            port
        ));
    }

    // Start new container
    let output = TokioCommand::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            &container_name,
            "-e",
            "POSTGRES_USER=reactor",
            "-e",
            "POSTGRES_PASSWORD=reactor",
            "-e",
            "POSTGRES_DB=reactor",
            "-p",
            &format!("{}:5432", port),
            "postgres:16-alpine",
        ])
        .output()
        .await
        .map_err(|e| CliError::ConfigError(format!("Failed to start Postgres container: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::ConfigError(format!(
            "Docker failed to start Postgres: {}",
            stderr
        )));
    }

    // Wait for Postgres to be ready
    for _ in 0..30 {
        let check = TokioCommand::new("docker")
            .args(["exec", &container_name, "pg_isready", "-U", "reactor"])
            .output()
            .await;

        if let Ok(out) = check {
            if out.status.success() {
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    Ok(format!(
        "postgres://reactor:reactor@localhost:{}/reactor",
        port
    ))
}
