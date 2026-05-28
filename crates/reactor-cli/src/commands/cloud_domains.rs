//! Cloud domains CLI commands.
//!
//! Commands for managing DNS subdomains on reactor.cloud via Cloudflare.

use crate::cli::{Cli, CloudDomainsArgs, CloudDomainsCommands};
use crate::cloudflare::{
    fqdn, normalize_subdomain, resolve_edge_target, resolve_token, resolve_zone, CloudflareClient,
};
use crate::error::{CliError, CliResult};
use crate::output::Output;

pub async fn run(cli: &Cli, args: &CloudDomainsArgs, output: &Output) -> CliResult<()> {
    match &args.command {
        CloudDomainsCommands::Add { name, target } => {
            add_domain(args.cf_token.as_deref(), name, target, output).await
        }
        CloudDomainsCommands::List => list_domains(args.cf_token.as_deref(), output).await,
        CloudDomainsCommands::Remove { name, yes } => {
            remove_domain(args.cf_token.as_deref(), name, *yes || cli.yes, output).await
        }
    }
}

async fn add_domain(
    cf_token: Option<&str>,
    name: &str,
    target: &str,
    output: &Output,
) -> CliResult<()> {
    let token = resolve_token(cf_token)?;
    let zone = resolve_zone();
    let label = normalize_subdomain(name, &zone);
    let full_name = fqdn(&label, &zone);

    output.info(&format!(
        "Creating CNAME: {} -> {}",
        full_name, target
    ));

    let client = CloudflareClient::new(token);
    let zone_id = client.get_zone_id(&zone).await?;

    let record = client
        .create_record(&zone_id, &full_name, target, false)
        .await?;

    if output.format().is_json() {
        output.success(&record)?;
    } else {
        use crate::output::human;
        human::print_section("DNS Record Created");
        human::print_kv("Name", &record.name);
        human::print_kv("Type", &record.record_type);
        human::print_kv("Target", &record.content);
        human::print_kv("Proxied", if record.proxied { "yes" } else { "no" });
        human::print_kv("ID", &record.id);
    }

    Ok(())
}

async fn list_domains(cf_token: Option<&str>, output: &Output) -> CliResult<()> {
    let token = resolve_token(cf_token)?;
    let zone = resolve_zone();

    let client = CloudflareClient::new(token);
    let zone_id = client.get_zone_id(&zone).await?;

    let records = client.list_records(&zone_id, None).await?;

    if output.format().is_json() {
        output.success(&records)?;
    } else if records.is_empty() {
        output.info(&format!("No CNAME records found in {}", zone));
    } else {
        let headers = &["NAME", "TARGET", "PROXIED", "ID"];
        let rows: Vec<Vec<String>> = records
            .iter()
            .map(|r| {
                vec![
                    r.name.clone(),
                    r.content.clone(),
                    if r.proxied { "yes" } else { "no" }.to_string(),
                    r.id.clone(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn remove_domain(
    cf_token: Option<&str>,
    name: &str,
    yes: bool,
    output: &Output,
) -> CliResult<()> {
    let token = resolve_token(cf_token)?;
    let zone = resolve_zone();
    let label = normalize_subdomain(name, &zone);
    let full_name = fqdn(&label, &zone);

    let client = CloudflareClient::new(token);
    let zone_id = client.get_zone_id(&zone).await?;

    // Find the record
    let records = client.list_records(&zone_id, Some(&full_name)).await?;
    let record = records.into_iter().find(|r| r.name == full_name);

    let record = match record {
        Some(r) => r,
        None => {
            return Err(CliError::User(format!(
                "DNS record '{}' not found",
                full_name
            )));
        }
    };

    // Confirm deletion
    if !yes && console::Term::stdout().is_term() {
        output.warning(&format!(
            "This will delete DNS record '{}'. Continue? [y/N]",
            full_name
        ));

        let confirm = console::Term::stdout()
            .read_line()
            .map(|s| s.trim().eq_ignore_ascii_case("y"))
            .unwrap_or(false);

        if !confirm {
            return Err(CliError::User("Aborted by user".into()));
        }
    } else if !yes {
        return Err(CliError::RequiresConfirmation);
    }

    client.delete_record(&zone_id, &record.id).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "deleted": full_name,
            "record_id": record.id,
        }))?;
    } else {
        output.success_message(&format!("DNS record '{}' deleted", full_name))?;
    }

    Ok(())
}

/// Provision a subdomain for use during project creation.
///
/// This is called from `cloud.rs::create_project` when `--subdomain` is passed.
/// It returns the FQDN of the created record, or an error.
pub async fn provision_subdomain(
    cf_token: Option<&str>,
    subdomain: &str,
    output: &Output,
) -> CliResult<String> {
    let token = resolve_token(cf_token)?;
    let zone = resolve_zone();
    let target = resolve_edge_target();
    let label = normalize_subdomain(subdomain, &zone);
    let full_name = fqdn(&label, &zone);

    output.info(&format!(
        "Provisioning subdomain: {} -> {}",
        full_name, target
    ));

    let client = CloudflareClient::new(token);
    let zone_id = client.get_zone_id(&zone).await?;

    client
        .create_record(&zone_id, &full_name, &target, false)
        .await?;

    Ok(full_name)
}
