//! Functions command implementation.

use crate::cli::{Cli, FunctionsArgs, FunctionsCommands, FunctionsEnvCommands};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, args: &FunctionsArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match &args.command {
        FunctionsCommands::List => list(&client, output).await,
        FunctionsCommands::Show { name } => show(&client, name, output).await,
        FunctionsCommands::Deploy { name, source } => {
            deploy(&client, name, source.as_deref(), output).await
        }
        FunctionsCommands::Rollback { name, to } => rollback(&client, name, to, output).await,
        FunctionsCommands::Invoke { name, data } => {
            invoke(&client, name, data.as_deref(), output).await
        }
        FunctionsCommands::Env(env_args) => match &env_args.command {
            FunctionsEnvCommands::List { name } => env_list(&client, name, output).await,
            FunctionsEnvCommands::Get { name, key } => env_get(&client, name, key, output).await,
            FunctionsEnvCommands::Set { name, key, value } => {
                env_set(&client, name, key, value, output).await
            }
            FunctionsEnvCommands::Unset { name, key } => env_unset(&client, name, key, output).await,
        },
        FunctionsCommands::Logs {
            name,
            since,
            follow,
        } => logs(&client, name, since.as_deref(), *follow, output).await,
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

async fn list(client: &Client, output: &Output) -> CliResult<()> {
    let functions = client.functions_list().await?;

    if output.format().is_json() {
        output.success(&functions)?;
    } else if functions.is_empty() {
        output.info("No functions found.");
    } else {
        let headers = &["NAME", "RUNTIME", "DEPLOYMENT", "UPDATED"];
        let rows: Vec<Vec<String>> = functions
            .iter()
            .map(|f| {
                vec![
                    f.name.clone(),
                    f.runtime.clone(),
                    f.current_deployment_id.map(|id| id.to_string()).unwrap_or_default(),
                    f.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn show(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let func = client.functions_get(name).await?;

    if output.format().is_json() {
        output.success(&func)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Function: {}", func.name));
        human::print_kv("ID", &func.id.to_string());
        human::print_kv("Runtime", &func.runtime);
        if let Some(deployment_id) = func.current_deployment_id {
            human::print_kv("Current Deployment", &deployment_id.to_string());
        }
        human::print_kv("Created", &func.created_at.to_rfc3339());
        human::print_kv("Updated", &func.updated_at.to_rfc3339());
    }

    Ok(())
}

async fn deploy(
    client: &Client,
    name: &str,
    source: Option<&std::path::Path>,
    output: &Output,
) -> CliResult<()> {
    // For now, just show a message - full implementation would zip source and upload
    let _ = (client, name, source);
    output.info("Function deploy not yet fully implemented. Use 'reactor deploy' for full project deployment.");
    Ok(())
}

async fn rollback(client: &Client, name: &str, to: &str, output: &Output) -> CliResult<()> {
    let deployment_id = to
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid deployment ID".into()))?;

    let func = client.functions_rollback(name, deployment_id).await?;

    if output.format().is_json() {
        output.success(&func)?;
    } else {
        output.success_message(&format!("Rolled back '{}' to deployment {}", name, to))?;
    }

    Ok(())
}

async fn invoke(
    client: &Client,
    name: &str,
    data: Option<&str>,
    output: &Output,
) -> CliResult<()> {
    let payload = if let Some(d) = data {
        if d.starts_with('@') {
            let path = &d[1..];
            let content = std::fs::read_to_string(path)?;
            serde_json::from_str(&content)?
        } else {
            serde_json::from_str(d)?
        }
    } else {
        None
    };

    let result = client.functions_invoke(name, payload).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        println!("Invocation ID: {}", result.invocation_id);
        println!("Status: {}", result.status);
        println!("Duration: {}ms", result.duration_ms);
        if let Some(response) = result.response {
            println!("Response: {}", serde_json::to_string_pretty(&response)?);
        }
        if let Some(error) = result.error {
            output.warning(&format!("Error: {}", error));
        }
    }

    Ok(())
}

async fn env_list(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let vars = client.functions_env_list(name).await?;

    if output.format().is_json() {
        output.success(&vars)?;
    } else if vars.is_empty() {
        output.info("No environment variables set.");
    } else {
        let headers = &["KEY", "VALUE"];
        let rows: Vec<Vec<String>> = vars.iter().map(|v| vec![v.key.clone(), v.value.clone()]).collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn env_get(client: &Client, name: &str, key: &str, output: &Output) -> CliResult<()> {
    let var = client.functions_env_get(name, key).await?;

    if output.format().is_json() {
        output.success(&var)?;
    } else {
        println!("{}={}", var.key, var.value);
    }

    Ok(())
}

async fn env_set(
    client: &Client,
    name: &str,
    key: &str,
    value: &str,
    output: &Output,
) -> CliResult<()> {
    let var = client.functions_env_set(name, key, value).await?;

    if output.format().is_json() {
        output.success(&var)?;
    } else {
        output.success_message(&format!("Set {}={}", key, value))?;
    }

    Ok(())
}

async fn env_unset(client: &Client, name: &str, key: &str, output: &Output) -> CliResult<()> {
    client.functions_env_unset(name, key).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "unset": key }))?;
    } else {
        output.success_message(&format!("Unset {}", key))?;
    }

    Ok(())
}

async fn logs(
    client: &Client,
    name: &str,
    since: Option<&str>,
    follow: bool,
    output: &Output,
) -> CliResult<()> {
    // Note: --follow uses polling at v0.1, SSE in v0.2
    let _ = follow; // TODO: implement polling loop

    let logs = client.functions_logs(name, since, Some(100)).await?;

    if output.format().is_json() {
        output.success(&logs)?;
    } else if logs.is_empty() {
        output.info("No logs found.");
    } else {
        for entry in logs {
            println!(
                "{} [{}] {}",
                entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                entry.level,
                entry.message
            );
        }
    }

    Ok(())
}
