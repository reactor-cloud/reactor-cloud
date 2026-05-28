//! `reactor down` — stop a detached local development server.

use crate::cli::{Cli, DownArgs};
use crate::commands::up::ServerState;
use crate::error::{CliError, CliResult};
use crate::output::Output;
use reactor_client::{Client, ClientConfig};
use std::process::{Command, Stdio};

pub async fn run(cli: &Cli, args: &DownArgs, output: &Output) -> CliResult<()> {
    let context_name = get_context_name(cli, &args.context)?;

    let state = ServerState::load(&context_name)?.ok_or_else(|| {
        CliError::ConfigError(format!(
            "No server running for context '{}'. Use 'reactor status' to check.",
            context_name
        ))
    })?;

    if !state.is_running() {
        output.info("Server process is not running. Cleaning up stale state...");
        ServerState::remove(&context_name)?;

        // Also try to clean up the postgres container
        cleanup_postgres(&context_name).await?;

        if output.format().is_json() {
            output.success(&serde_json::json!({
                "status": "cleaned_up",
                "context": context_name
            }))?;
        } else {
            output.success_message("Cleaned up stale server state.")?;
        }
        return Ok(());
    }

    output.info(&format!("Stopping server (PID: {})...", state.pid));

    // Try graceful shutdown via API first
    let endpoint = url::Url::parse(&format!("http://{}", state.bind_addr))
        .map_err(|e| CliError::ConfigError(format!("Invalid bind address: {}", e)))?;
    let client_result = Client::new(ClientConfig::new(endpoint));
    if let Ok(client) = client_result {
        let _ = client.admin_shutdown().await;
        // Give it time to shut down gracefully
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    // If still running, force stop
    if state.is_running() {
        output.info("Forcing shutdown...");
        stop_process(state.pid)?;
    }

    // Clean up state file
    ServerState::remove(&context_name)?;

    // Clean up postgres container if ephemeral
    cleanup_postgres(&context_name).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "status": "stopped",
            "pid": state.pid,
            "context": context_name
        }))?;
    } else {
        output.success_message(&format!("Server stopped (was PID: {}).", state.pid))?;
    }

    Ok(())
}

fn get_context_name(cli: &Cli, args_context: &Option<String>) -> CliResult<String> {
    if let Some(ctx) = args_context {
        return Ok(ctx.clone());
    }
    if let Some(ctx) = &cli.context {
        return Ok(ctx.clone());
    }

    // Try to get from global config
    let config = crate::context::GlobalConfig::load()?;
    config.active_context.ok_or_else(|| {
        CliError::ConfigError("No context specified. Use --context or set an active context.".into())
    })
}

fn stop_process(pid: u32) -> CliResult<()> {
    #[cfg(unix)]
    {
        // Send SIGTERM first
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        // Wait a bit
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Check if still running, then SIGKILL
        let check = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if check.map(|s| s.success()).unwrap_or(false) {
            let _ = Command::new("kill")
                .args(["-9", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    Ok(())
}

async fn cleanup_postgres(context: &str) -> CliResult<()> {
    use tokio::process::Command as TokioCommand;

    let container_name = format!("reactor-dev-postgres-{}", context);

    // Stop and remove container (ignore errors)
    let _ = TokioCommand::new("docker")
        .args(["stop", &container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    let _ = TokioCommand::new("docker")
        .args(["rm", &container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    Ok(())
}
