//! Unified logs polling multiplexer command.

use crate::cli::{Cli, LogsArgs};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};
use serde::{Deserialize, Serialize};

/// Unified log entry for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedLogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
}

pub async fn run(cli: &Cli, args: &LogsArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    let logs: Vec<UnifiedLogEntry> = match args.capability.as_str() {
        "function" | "fn" | "func" => {
            let name = args.name.as_ref().ok_or_else(|| {
                CliError::InvalidArgument("Function name required for function logs.".into())
            })?;
            let entries = client.functions_logs(name, args.since.as_deref(), Some(100)).await?;
            entries.into_iter().map(|e| UnifiedLogEntry {
                timestamp: e.timestamp,
                level: e.level,
                message: e.message,
            }).collect()
        }
        "site" => {
            let name = args.name.as_ref().ok_or_else(|| {
                CliError::InvalidArgument("Site name required for site logs.".into())
            })?;
            let entries = client.sites_logs(name, args.since.as_deref(), Some(100)).await?;
            entries.into_iter().map(|e| UnifiedLogEntry {
                timestamp: e.timestamp,
                level: e.level,
                message: e.message,
            }).collect()
        }
        "job" => {
            let name = args.name.as_ref().ok_or_else(|| {
                CliError::InvalidArgument("Job name required for job logs.".into())
            })?;
            let entries = client.jobs_logs(name, args.since.as_deref(), Some(100)).await?;
            entries.into_iter().map(|e| UnifiedLogEntry {
                timestamp: e.timestamp,
                level: e.level,
                message: e.message,
            }).collect()
        }
        "server" | "system" => {
            let entries = client.admin_logs(args.since.as_deref(), Some(100)).await?;
            entries.into_iter().map(|e| UnifiedLogEntry {
                timestamp: e.timestamp,
                level: e.level,
                message: e.message,
            }).collect()
        }
        "all" => {
            // Get server logs as a fallback
            let entries = client.admin_logs(args.since.as_deref(), Some(100)).await
                .unwrap_or_default();
            entries.into_iter().map(|e| UnifiedLogEntry {
                timestamp: e.timestamp,
                level: e.level,
                message: e.message,
            }).collect()
        }
        cap => {
            return Err(CliError::InvalidArgument(format!(
                "Unknown capability '{}'. Valid: function, site, job, server, all",
                cap
            )));
        }
    };

    if output.format().is_json() {
        output.success(&logs)?;
    } else if logs.is_empty() {
        output.info("No logs found.");
    } else {
        for entry in logs {
            let level_color = match entry.level.to_lowercase().as_str() {
                "error" => console::Style::new().red(),
                "warn" | "warning" => console::Style::new().yellow(),
                "debug" => console::Style::new().dim(),
                _ => console::Style::new(),
            };

            println!(
                "{} [{}] {}",
                entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                level_color.apply_to(&entry.level),
                entry.message
            );
        }
    }

    // TODO: If --follow, implement polling loop
    if args.follow {
        output.info("Note: --follow polling not yet implemented. Showing latest logs only.");
    }

    Ok(())
}

fn build_client(cli: &Cli) -> CliResult<Client> {
    let config = GlobalConfig::load()?;
    let cwd = std::env::current_dir()?;
    let project = Project::try_resolve(&cwd, cli.manifest.as_deref());
    let project_default = project.as_ref().and_then(|p| p.manifest.default_context.as_deref());

    let resolved = resolve_context(&config, cli.context.as_deref(), project_default, cli.token.as_deref())?;

    let mut client_config = ClientConfig::new(resolved.endpoint);
    if let Some(token) = resolved.token {
        client_config = client_config.with_token(token);
    }
    if let Some(org) = resolved.org {
        client_config = client_config.with_org(org);
    }

    Client::new(client_config).map_err(Into::into)
}
