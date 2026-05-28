//! Sites command implementation.

use crate::cli::{Cli, SitesArgs, SitesCommands, SitesDomainsCommands};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, args: &SitesArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match &args.command {
        SitesCommands::List => list(&client, output).await,
        SitesCommands::Show { name } => show(&client, name, output).await,
        SitesCommands::Deploy { name, source } => deploy(&client, name, source.as_deref(), output).await,
        SitesCommands::Promote { name, deployment } => promote(&client, name, deployment, output).await,
        SitesCommands::Rollback { name } => rollback(&client, name, output).await,
        SitesCommands::Domains(domains_args) => match &domains_args.command {
            SitesDomainsCommands::List { name } => domains_list(&client, name, output).await,
            SitesDomainsCommands::Add { name, domain } => domains_add(&client, name, domain, output).await,
            SitesDomainsCommands::Remove { name, domain_id } => domains_remove(&client, name, domain_id, output).await,
            SitesDomainsCommands::Verify { name, domain_id } => domains_verify(&client, name, domain_id, output).await,
        },
        SitesCommands::Revalidate { name, path } => revalidate(&client, name, path, output).await,
        SitesCommands::Logs { name, since } => logs(&client, name, since.as_deref(), output).await,
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
    let sites = client.sites_list().await?;

    if output.format().is_json() {
        output.success(&sites)?;
    } else if sites.is_empty() {
        output.info("No sites found.");
    } else {
        let headers = &["NAME", "FRAMEWORK", "DEPLOYMENT", "UPDATED"];
        let rows: Vec<Vec<String>> = sites
            .iter()
            .map(|s| {
                vec![
                    s.name.clone(),
                    s.framework.clone(),
                    s.current_deployment_id.map(|id| id.to_string()).unwrap_or_default(),
                    s.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn show(client: &Client, name: &str, output: &Output) -> CliResult<()> {
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

async fn deploy(client: &Client, name: &str, source: Option<&std::path::Path>, output: &Output) -> CliResult<()> {
    let _ = (client, name, source);
    output.info("Site deploy not yet fully implemented. Use 'reactor deploy' for full project deployment.");
    Ok(())
}

async fn promote(client: &Client, name: &str, deployment: &str, output: &Output) -> CliResult<()> {
    let deployment_id = deployment
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid deployment ID".into()))?;

    let site = client.sites_promote(name, deployment_id).await?;

    if output.format().is_json() {
        output.success(&site)?;
    } else {
        output.success_message(&format!("Promoted deployment {} for '{}'", deployment, name))?;
    }

    Ok(())
}

async fn rollback(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let site = client.sites_rollback(name).await?;

    if output.format().is_json() {
        output.success(&site)?;
    } else {
        output.success_message(&format!("Rolled back '{}'", name))?;
    }

    Ok(())
}

async fn domains_list(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let domains = client.sites_domains_list(name).await?;

    if output.format().is_json() {
        output.success(&domains)?;
    } else if domains.is_empty() {
        output.info("No custom domains configured.");
    } else {
        let headers = &["DOMAIN", "STATUS", "ID"];
        let rows: Vec<Vec<String>> = domains
            .iter()
            .map(|d| vec![d.domain.clone(), format!("{:?}", d.status).to_lowercase(), d.id.to_string()])
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn domains_add(client: &Client, name: &str, domain: &str, output: &Output) -> CliResult<()> {
    let result = client.sites_domain_add(name, domain).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        output.success_message(&format!("Added domain '{}' to site '{}'", domain, name))?;
    }

    Ok(())
}

async fn domains_remove(client: &Client, name: &str, domain_id: &str, output: &Output) -> CliResult<()> {
    let id = domain_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid domain ID".into()))?;
    client.sites_domain_remove(name, id).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "removed": domain_id }))?;
    } else {
        output.success_message("Domain removed.")?;
    }

    Ok(())
}

async fn domains_verify(client: &Client, name: &str, domain_id: &str, output: &Output) -> CliResult<()> {
    let id = domain_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid domain ID".into()))?;
    let result = client.sites_domain_verify(name, id).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        output.success_message(&format!("Domain verification: {:?}", result.status))?;
    }

    Ok(())
}

async fn revalidate(client: &Client, name: &str, path: &str, output: &Output) -> CliResult<()> {
    client.sites_revalidate(name, path).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "revalidated": path }))?;
    } else {
        output.success_message(&format!("Revalidated path '{}' for site '{}'", path, name))?;
    }

    Ok(())
}

async fn logs(client: &Client, name: &str, since: Option<&str>, output: &Output) -> CliResult<()> {
    let logs = client.sites_logs(name, since, Some(100)).await?;

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
