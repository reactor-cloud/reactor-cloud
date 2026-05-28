//! `reactor status` — check the status of local development servers.

use crate::cli::{Cli, StatusArgs};
use crate::commands::up::ServerState;
use crate::context::GlobalConfig;
use crate::error::{CliError, CliResult};
use crate::output::Output;
use chrono::Utc;
use std::fs;

pub async fn run(cli: &Cli, args: &StatusArgs, output: &Output) -> CliResult<()> {
    if args.all {
        // List status of all known server contexts
        list_all_servers(output).await
    } else {
        // Show status for a specific context
        let context_name = get_context_name(cli, &args.context)?;
        show_server_status(&context_name, output).await
    }
}

fn get_context_name(cli: &Cli, args_context: &Option<String>) -> CliResult<String> {
    if let Some(ctx) = args_context {
        return Ok(ctx.clone());
    }
    if let Some(ctx) = &cli.context {
        return Ok(ctx.clone());
    }

    let config = GlobalConfig::load()?;
    config.active_context.ok_or_else(|| {
        CliError::ConfigError("No context specified. Use --context, --all, or set an active context.".into())
    })
}

async fn show_server_status(context: &str, output: &Output) -> CliResult<()> {
    match ServerState::load(context)? {
        Some(state) => {
            let is_running = state.is_running();
            let uptime = if is_running {
                let duration = Utc::now().signed_duration_since(state.started_at);
                format_duration(duration)
            } else {
                "stopped".to_string()
            };

            if output.format().is_json() {
                output.success(&serde_json::json!({
                    "context": context,
                    "running": is_running,
                    "pid": state.pid,
                    "bind_addr": state.bind_addr,
                    "database_url": state.database_url,
                    "project_root": state.project_root,
                    "started_at": state.started_at,
                    "uptime": uptime
                }))?;
            } else {
                use crate::output::human;
                human::print_section(&format!("Server: {}", context));

                let status = if is_running {
                    console::style("running").green().to_string()
                } else {
                    console::style("stopped").red().to_string()
                };

                human::print_kv("Status", &status);
                human::print_kv("PID", &state.pid.to_string());
                human::print_kv("Address", &format!("http://{}", state.bind_addr));
                human::print_kv("Database", &state.database_url);
                human::print_kv("Project", &state.project_root.display().to_string());
                human::print_kv("Started", &state.started_at.to_rfc3339());
                human::print_kv("Uptime", &uptime);
            }
        }
        None => {
            if output.format().is_json() {
                output.success(&serde_json::json!({
                    "context": context,
                    "running": false,
                    "message": "No server state found"
                }))?;
            } else {
                output.info(&format!("No server running for context '{}'.", context));
            }
        }
    }

    Ok(())
}

async fn list_all_servers(output: &Output) -> CliResult<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| CliError::ConfigError("Cannot find home directory".into()))?;
    let reactor_dir = home.join(".reactor");

    if !reactor_dir.exists() {
        if output.format().is_json() {
            output.success(&serde_json::json!({ "servers": [] }))?;
        } else {
            output.info("No local servers configured.");
        }
        return Ok(());
    }

    let mut servers = Vec::new();

    for entry in fs::read_dir(&reactor_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let state_file = path.join("state.json");
            if state_file.exists() {
                let context_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                if let Ok(Some(state)) = ServerState::load(&context_name) {
                    let is_running = state.is_running();
                    let uptime = if is_running {
                        let duration = Utc::now().signed_duration_since(state.started_at);
                        format_duration(duration)
                    } else {
                        "stopped".to_string()
                    };

                    servers.push(serde_json::json!({
                        "context": context_name,
                        "running": is_running,
                        "pid": state.pid,
                        "bind_addr": state.bind_addr,
                        "uptime": uptime
                    }));
                }
            }
        }
    }

    if output.format().is_json() {
        output.success(&serde_json::json!({ "servers": servers }))?;
    } else if servers.is_empty() {
        output.info("No local servers found.");
    } else {
        let headers = &["CONTEXT", "STATUS", "PID", "ADDRESS", "UPTIME"];
        let rows: Vec<Vec<String>> = servers
            .iter()
            .map(|s| {
                let running = s["running"].as_bool().unwrap_or(false);
                vec![
                    s["context"].as_str().unwrap_or("").to_string(),
                    if running { "running" } else { "stopped" }.to_string(),
                    s["pid"].as_u64().map(|p| p.to_string()).unwrap_or_default(),
                    format!("http://{}", s["bind_addr"].as_str().unwrap_or("")),
                    s["uptime"].as_str().unwrap_or("").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

fn format_duration(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h {}m", hours, mins)
    } else {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        format!("{}d {}h", days, hours)
    }
}
