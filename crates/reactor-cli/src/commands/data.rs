//! Data command implementation.

use crate::cli::{Cli, DataArgs, DataCommands};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;
use crate::confirm;
use reactor_client::{Client, ClientConfig};
use std::collections::HashMap;

pub async fn run(cli: &Cli, args: &DataArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match &args.command {
        DataCommands::Migrate { dry_run } => migrate(&client, *dry_run, output).await,
        DataCommands::Inspect { table } => inspect(&client, table, output).await,
        DataCommands::Query { sql, params, write } => {
            query(cli, &client, sql, params.as_deref(), *write, output).await
        }
    }
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

async fn migrate(client: &Client, dry_run: bool, output: &Output) -> CliResult<()> {
    let result = client.data_migrate(dry_run).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        if result.applied.is_empty() && result.pending.is_empty() {
            output.info("No migrations to apply.");
        } else {
            if !result.applied.is_empty() {
                if dry_run {
                    output.info("Would apply:");
                } else {
                    output.info("Applied:");
                }
                for m in &result.applied {
                    output.info(&format!("  - {}", m));
                }
            }
            if !result.pending.is_empty() {
                output.info("Pending:");
                for m in &result.pending {
                    output.info(&format!("  - {}", m));
                }
            }
        }
    }

    Ok(())
}

async fn inspect(client: &Client, table: &str, output: &Output) -> CliResult<()> {
    let info = client.data_inspect(table).await?;

    if output.format().is_json() {
        output.success(&info)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Table: {}.{}", info.schema, info.name));
        if let Some(count) = info.row_count {
            human::print_kv("Row Count", &count.to_string());
        }

        human::print_section("Columns");
        let headers = &["NAME", "TYPE", "NULLABLE", "PRIMARY KEY"];
        let rows: Vec<Vec<String>> = info
            .columns
            .iter()
            .map(|c| {
                vec![
                    c.name.clone(),
                    c.data_type.clone(),
                    if c.is_nullable { "yes" } else { "no" }.to_string(),
                    if c.is_primary_key { "yes" } else { "no" }.to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn query(
    cli: &Cli,
    client: &Client,
    sql: &str,
    params: Option<&str>,
    write: bool,
    output: &Output,
) -> CliResult<()> {
    // Parse params if provided
    let params_map: Option<HashMap<String, serde_json::Value>> = if let Some(p) = params {
        if p.starts_with('@') {
            let path = &p[1..];
            let content = std::fs::read_to_string(path)?;
            Some(serde_json::from_str(&content)?)
        } else {
            Some(serde_json::from_str(p)?)
        }
    } else {
        None
    };

    // Require confirmation for write operations
    if write {
        confirm(cli, "This will execute a write operation. Continue?")?;
    }

    let result = client.data_query(sql, params_map, write).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        if result.columns.is_empty() {
            if let Some(rows_affected) = result.rows_affected {
                output.success_message(&format!("{} rows affected.", rows_affected))?;
            } else {
                output.success_message("Query executed.")?;
            }
        } else {
            // Print result table
            let headers: Vec<&str> = result.columns.iter().map(|s| s.as_str()).collect();
            let rows: Vec<Vec<String>> = result
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|v| match v {
                            serde_json::Value::Null => "NULL".to_string(),
                            serde_json::Value::String(s) => s.clone(),
                            _ => v.to_string(),
                        })
                        .collect()
                })
                .collect();
            output.table(&headers, rows)?;

            output.info(&format!("{} rows returned.", result.rows.len()));
        }
    }

    Ok(())
}
