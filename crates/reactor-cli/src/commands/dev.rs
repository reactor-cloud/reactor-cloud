//! `reactor dev` — embedded local development server.
//!
//! This module is only available when the `dev` feature is enabled.

use crate::cli::{Cli, DevArgs};
use crate::context::{save_context_config, AuthConfig, ContextConfig, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub async fn run(cli: &Cli, args: &DevArgs, output: &Output) -> CliResult<()> {
    #[cfg(not(feature = "dev"))]
    {
        let _ = (cli, args, output);
        return Err(CliError::FeatureDisabled(
            "The 'dev' feature is not enabled. Rebuild with `cargo build --features dev` or install the full CLI.".into(),
        ));
    }

    #[cfg(feature = "dev")]
    {
        run_dev(cli, args, output).await
    }
}

#[cfg(feature = "dev")]
async fn run_dev(cli: &Cli, args: &DevArgs, output: &Output) -> CliResult<()> {
    // Resolve project
    let cwd = std::env::current_dir()?;
    let _project = Project::try_resolve(&cwd, cli.manifest.as_deref()).ok_or_else(|| {
        CliError::ConfigError("No reactor.toml found. Run 'reactor init' first.".into())
    })?;

    // Determine database URL
    let database_url = if args.ephemeral {
        output.info("Starting ephemeral Docker Postgres container...");
        start_ephemeral_postgres(args.port + 1000).await?
    } else if let Some(db) = &args.db {
        db.clone()
    } else if let Ok(url) = std::env::var("DATABASE_URL") {
        url
    } else {
        output.info("Starting ephemeral Docker Postgres container...");
        start_ephemeral_postgres(args.port + 1000).await?
    };

    // Build server config
    let bind_addr = format!("{}:{}", args.host, args.port);
    output.info(&format!("Starting Reactor dev server on http://{}", bind_addr));
    output.info(&format!("Using database: {}", database_url));

    // Auto-create/update local context
    let context_name = args.context.clone().unwrap_or_else(|| "local".to_string());
    create_or_update_local_context(&context_name, &bind_addr, args.admin_token.as_deref())?;
    output.info(&format!("Using context '{}'", context_name));

    // Setup SIGINT handler
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    ctrlc::set_handler(move || {
        eprintln!("\nShutting down...");
        shutdown_clone.store(true, Ordering::SeqCst);
    })
    .map_err(|e| CliError::ConfigError(format!("Failed to set Ctrl+C handler: {}", e)))?;

    output.success_message(&format!(
        "Server running at http://{}. Press Ctrl+C to stop.",
        bind_addr
    ))?;

    // TODO: When reactor-server is ready, start it here
    // For now, just wait for shutdown signal
    output.warning("Note: Embedded server not yet implemented. Waiting for Ctrl+C...");

    // Wait for shutdown signal
    while !shutdown.load(Ordering::SeqCst) {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Cleanup ephemeral container if needed
    if args.ephemeral {
        cleanup_ephemeral_postgres().await?;
    }

    output.success_message("Server stopped.")?;
    Ok(())
}

#[cfg(feature = "dev")]
fn create_or_update_local_context(
    name: &str,
    bind_addr: &str,
    admin_token: Option<&str>,
) -> CliResult<()> {
    let mut config = GlobalConfig::load()?;

    let endpoint = format!("http://{}", bind_addr);
    let auth = if admin_token.is_some() {
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

    // Make this the active context
    config.active_context = Some(name.to_string());

    config.save()?;
    save_context_config(&config, name)?;

    Ok(())
}

#[cfg(feature = "dev")]
async fn start_ephemeral_postgres(port: u16) -> CliResult<String> {
    use std::process::Stdio;
    use tokio::process::Command;

    let container_name = "reactor-dev-postgres";

    // Check if container already exists
    let check = Command::new("docker")
        .args(["ps", "-q", "-f", &format!("name={}", container_name)])
        .output()
        .await
        .map_err(|e| CliError::ConfigError(format!("Docker not available: {}", e)))?;

    if !check.stdout.is_empty() {
        // Container already running
        return Ok(format!(
            "postgres://reactor:reactor@localhost:{}/reactor",
            port
        ));
    }

    // Start new container
    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            container_name,
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
        let check = Command::new("docker")
            .args(["exec", container_name, "pg_isready", "-U", "reactor"])
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

#[cfg(feature = "dev")]
async fn cleanup_ephemeral_postgres() -> CliResult<()> {
    use tokio::process::Command;

    let container_name = "reactor-dev-postgres";

    // Stop and remove container
    let _ = Command::new("docker")
        .args(["stop", container_name])
        .output()
        .await;

    let _ = Command::new("docker")
        .args(["rm", container_name])
        .output()
        .await;

    Ok(())
}
