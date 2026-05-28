//! Uniform resource inspection command.

use crate::cli::{Cli, InspectArgs};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, args: &InspectArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match args.kind.as_str() {
        "function" | "fn" | "func" => inspect_function(&client, &args.name, output).await,
        "site" => inspect_site(&client, &args.name, output).await,
        "job" => inspect_job(&client, &args.name, output).await,
        "table" => inspect_table(&client, &args.name, output).await,
        "org" | "organization" => inspect_org(&client, &args.name, output).await,
        kind => Err(CliError::InvalidArgument(format!(
            "Unknown resource kind '{}'. Valid kinds: function, site, job, table, org",
            kind
        ))),
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

async fn inspect_function(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let func = client.functions_get(name).await?;

    if output.format().is_json() {
        output.success(&func)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Function: {}", func.name));
        human::print_kv("ID", &func.id.to_string());
        human::print_kv("Runtime", &func.runtime);
        human::print_kv("Status", &func.status);
        human::print_kv("Memory", &format!("{} MB", func.memory_mb));
        human::print_kv("Timeout", &format!("{} s", func.timeout_sec));
        if let Some(deployment_id) = func.current_deployment_id {
            human::print_kv("Current Deployment", &deployment_id.to_string());
        }
        human::print_kv("Created", &func.created_at.to_rfc3339());
        human::print_kv("Updated", &func.updated_at.to_rfc3339());
    }

    Ok(())
}

async fn inspect_site(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let site = client.sites_get(name).await?;

    if output.format().is_json() {
        output.success(&site)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Site: {}", site.name));
        human::print_kv("ID", &site.id.to_string());
        human::print_kv("Framework", &site.framework);
        if let Some(deployment_id) = site.current_deployment_id {
            human::print_kv("Current Deployment", &deployment_id.to_string());
        }
        human::print_kv("Created", &site.created_at.to_rfc3339());
        human::print_kv("Updated", &site.updated_at.to_rfc3339());
    }

    Ok(())
}

async fn inspect_job(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let job = client.jobs_get(name).await?;

    if output.format().is_json() {
        output.success(&job)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Job: {}", job.name));
        human::print_kv("ID", &job.id.to_string());
        human::print_kv("Function", &job.function_name);
        human::print_kv("Status", &job.status);
        human::print_kv("Created", &job.created_at.to_rfc3339());
        human::print_kv("Updated", &job.updated_at.to_rfc3339());
    }

    Ok(())
}

async fn inspect_table(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let info = client.data_inspect(name).await?;

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

async fn inspect_org(client: &Client, org_id: &str, output: &Output) -> CliResult<()> {
    let id = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let org = client.auth_org_get(id).await?;

    if output.format().is_json() {
        output.success(&org)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Organization: {}", org.name));
        human::print_kv("ID", &org.id.to_string());
        human::print_kv("Slug", &org.slug);
        human::print_kv("Created", &org.created_at.to_rfc3339());
        human::print_kv("Updated", &org.updated_at.to_rfc3339());
    }

    Ok(())
}
