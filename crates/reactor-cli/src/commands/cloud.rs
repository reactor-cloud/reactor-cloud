//! Cloud control plane CLI commands.
//!
//! Commands for managing Reactor Cloud projects, members, keys, and audit logs.

use crate::cli::{Cli, CloudArgs, CloudCommands, CloudKeysCommands, CloudMembersCommands, CloudProjectsCommands};
use crate::commands::cloud_domains;
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use reactor_client::{Client, ClientConfig};
use uuid::Uuid;

pub async fn run(cli: &Cli, args: &CloudArgs, output: &Output) -> CliResult<()> {
    match &args.command {
        CloudCommands::Projects(projects_args) => {
            let client = build_client(cli)?;
            match &projects_args.command {
                CloudProjectsCommands::Create {
                    name,
                    region,
                    subdomain,
                    cf_token,
                } => {
                    create_project(
                        &client,
                        name,
                        region.as_deref(),
                        subdomain.as_deref(),
                        cf_token.as_deref(),
                        output,
                    )
                    .await
                }
                CloudProjectsCommands::List { owner, limit, offset } => {
                    list_projects(&client, *owner, *limit, *offset, output).await
                }
                CloudProjectsCommands::Show { project_ref } => {
                    show_project(&client, project_ref, output).await
                }
                CloudProjectsCommands::Delete { project_ref, yes } => {
                    delete_project(&client, project_ref, *yes || cli.yes, output).await
                }
            }
        }
        CloudCommands::Keys(keys_args) => {
            let client = build_client(cli)?;
            match &keys_args.command {
                CloudKeysCommands::List { project_ref } => {
                    list_keys(&client, project_ref, output).await
                }
                CloudKeysCommands::Create { project_ref, kind } => {
                    create_key(&client, project_ref, kind, output).await
                }
                CloudKeysCommands::Rotate { project_ref, key_id } => {
                    rotate_key(&client, project_ref, key_id, output).await
                }
                CloudKeysCommands::Revoke { project_ref, key_id } => {
                    revoke_key(&client, project_ref, key_id, output).await
                }
            }
        }
        CloudCommands::Members(members_args) => {
            let client = build_client(cli)?;
            match &members_args.command {
                CloudMembersCommands::List { project_ref } => {
                    list_members(&client, project_ref, output).await
                }
                CloudMembersCommands::Add {
                    project_ref,
                    user_id,
                    role,
                } => add_member(&client, project_ref, user_id, role, output).await,
                CloudMembersCommands::Remove { project_ref, user_id } => {
                    remove_member(&client, project_ref, user_id, output).await
                }
            }
        }
        CloudCommands::Domains(domains_args) => {
            cloud_domains::run(cli, domains_args, output).await
        }
        CloudCommands::Audit { project_ref, limit } => {
            let client = build_client(cli)?;
            get_audit(&client, project_ref, *limit, output).await
        }
    }
}

fn build_client(cli: &Cli) -> CliResult<Client> {
    let config = GlobalConfig::load()?;
    let resolved = resolve_context(&config, cli.context.as_deref(), None, cli.token.as_deref())?;

    let mut client_config = ClientConfig::new(resolved.endpoint);
    if let Some(token) = resolved.token {
        client_config = client_config.with_token(token);
    }
    if let Some(org) = resolved.org {
        client_config = client_config.with_org(org);
    }

    Client::new(client_config).map_err(Into::into)
}

// ============================================================================
// Projects
// ============================================================================

async fn create_project(
    client: &Client,
    name: &str,
    region: Option<&str>,
    subdomain: Option<&str>,
    cf_token: Option<&str>,
    output: &Output,
) -> CliResult<()> {
    // For now, use a fixed owner_user_id (in production this would come from auth)
    let owner_user_id = Uuid::nil();

    let result = client
        .cloud_projects_create(name, region, owner_user_id)
        .await?;

    // Attempt DNS provisioning if --subdomain was provided
    let dns_result = if let Some(sub) = subdomain {
        match cloud_domains::provision_subdomain(cf_token, sub, output).await {
            Ok(fqdn) => Some(Ok(fqdn)),
            Err(e) => Some(Err(e)),
        }
    } else {
        None
    };

    if output.format().is_json() {
        // Include DNS result in JSON output
        let mut json_result = serde_json::to_value(&result)?;
        if let Some(ref dns) = dns_result {
            match dns {
                Ok(fqdn) => {
                    json_result["subdomain"] = serde_json::json!({
                        "success": true,
                        "fqdn": fqdn,
                    });
                }
                Err(e) => {
                    json_result["subdomain"] = serde_json::json!({
                        "success": false,
                        "error": e.to_string(),
                    });
                }
            }
        }
        output.success(&json_result)?;
    } else {
        use crate::output::human;
        human::print_section("Project Created");
        human::print_kv("ID", &result.project.id.to_string());
        human::print_kv("Ref", &result.project.project_ref);
        human::print_kv("Name", &result.project.name);
        human::print_kv("Hostname", &result.project.hostname);
        human::print_kv("Status", &result.project.status);
        human::print_kv("Region", &result.project.region);

        // Show DNS result
        if let Some(ref dns) = dns_result {
            println!();
            match dns {
                Ok(fqdn) => {
                    human::print_section("Subdomain");
                    human::print_kv("FQDN", fqdn);
                    human::print_kv("Status", "created");
                }
                Err(e) => {
                    output.warning(&format!("DNS provisioning failed: {}", e));
                    output.info("Retry manually with: reactor cloud domains add <subdomain>");
                }
            }
        }

        println!();
        human::print_section("API Keys");
        human::print_kv("Anon Key", &result.anon_key);
        human::print_kv("Service Key", &result.service_key);
        println!();
        output.warning("Save these keys now. They won't be shown again.");
    }

    Ok(())
}

async fn list_projects(
    client: &Client,
    owner: Option<Uuid>,
    limit: i32,
    offset: i32,
    output: &Output,
) -> CliResult<()> {
    let projects = client
        .cloud_projects_list(owner, Some(limit), Some(offset))
        .await?;

    if output.format().is_json() {
        output.success(&projects)?;
    } else if projects.is_empty() {
        output.info("No projects found.");
    } else {
        let headers = &["REF", "NAME", "STATUS", "REGION", "HOSTNAME"];
        let rows: Vec<Vec<String>> = projects
            .iter()
            .map(|p| {
                vec![
                    p.project_ref.clone(),
                    p.name.clone(),
                    p.status.clone(),
                    p.region.clone(),
                    p.hostname.clone(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn show_project(client: &Client, project_ref: &str, output: &Output) -> CliResult<()> {
    let project = client.cloud_projects_get(project_ref).await?;

    if output.format().is_json() {
        output.success(&project)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Project: {}", project.name));
        human::print_kv("ID", &project.id.to_string());
        human::print_kv("Ref", &project.project_ref);
        human::print_kv("Hostname", &project.hostname);
        human::print_kv("Status", &project.status);
        human::print_kv("Region", &project.region);
        human::print_kv("Backend", &project.backend_kind);
        human::print_kv("Owner", &project.owner_user_id.to_string());
        human::print_kv("Created", &project.created_at.to_rfc3339());
        human::print_kv("Updated", &project.updated_at.to_rfc3339());
    }

    Ok(())
}

async fn delete_project(
    client: &Client,
    project_ref: &str,
    yes: bool,
    output: &Output,
) -> CliResult<()> {
    if !yes && console::Term::stdout().is_term() {
        let confirm = console::Term::stdout()
            .read_line()
            .map(|s| s.trim().eq_ignore_ascii_case("y"))
            .unwrap_or(false);

        output.warning(&format!(
            "This will delete project '{}'. Continue? [y/N]",
            project_ref
        ));
        if !confirm {
            return Err(CliError::User("Aborted by user".into()));
        }
    } else if !yes {
        return Err(CliError::RequiresConfirmation);
    }

    let project = client.cloud_projects_delete(project_ref).await?;

    if output.format().is_json() {
        output.success(&project)?;
    } else {
        output.success_message(&format!(
            "Project '{}' scheduled for deletion (status: {})",
            project_ref, project.status
        ))?;
    }

    Ok(())
}

// ============================================================================
// Keys
// ============================================================================

async fn list_keys(client: &Client, project_ref: &str, output: &Output) -> CliResult<()> {
    let keys = client.cloud_keys_list(project_ref).await?;

    if output.format().is_json() {
        output.success(&keys)?;
    } else if keys.is_empty() {
        output.info("No keys found.");
    } else {
        let headers = &["ID", "KIND", "ACTIVE", "CREATED"];
        let rows: Vec<Vec<String>> = keys
            .iter()
            .map(|k| {
                vec![
                    k.id.to_string(),
                    k.kind.clone(),
                    if k.active { "yes" } else { "no" }.to_string(),
                    k.created_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn create_key(
    client: &Client,
    project_ref: &str,
    kind: &str,
    output: &Output,
) -> CliResult<()> {
    let result = client.cloud_keys_create(project_ref, kind).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        use crate::output::human;
        human::print_section("Key Created");
        human::print_kv("ID", &result.key.id.to_string());
        human::print_kv("Kind", &result.key.kind);
        human::print_kv("Value", &result.value);
        println!();
        output.warning("Save this key now. It won't be shown again.");
    }

    Ok(())
}

async fn rotate_key(
    client: &Client,
    project_ref: &str,
    key_id: &str,
    output: &Output,
) -> CliResult<()> {
    let key_uuid = key_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid key ID".into()))?;

    let result = client.cloud_keys_rotate(project_ref, key_uuid).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        use crate::output::human;
        human::print_section("Key Rotated");
        human::print_kv("New ID", &result.key.id.to_string());
        human::print_kv("Kind", &result.key.kind);
        human::print_kv("Value", &result.value);
        println!();
        output.warning("Save this key now. It won't be shown again.");
    }

    Ok(())
}

async fn revoke_key(
    client: &Client,
    project_ref: &str,
    key_id: &str,
    output: &Output,
) -> CliResult<()> {
    let key_uuid = key_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid key ID".into()))?;

    client.cloud_keys_revoke(project_ref, key_uuid).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "revoked": key_id }))?;
    } else {
        output.success_message(&format!("Key {} revoked", key_id))?;
    }

    Ok(())
}

// ============================================================================
// Members
// ============================================================================

async fn list_members(client: &Client, project_ref: &str, output: &Output) -> CliResult<()> {
    let members = client.cloud_members_list(project_ref).await?;

    if output.format().is_json() {
        output.success(&members)?;
    } else if members.is_empty() {
        output.info("No members found.");
    } else {
        let headers = &["USER ID", "ROLE", "ADDED"];
        let rows: Vec<Vec<String>> = members
            .iter()
            .map(|m| {
                vec![
                    m.user_id.to_string(),
                    m.role.clone(),
                    m.created_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn add_member(
    client: &Client,
    project_ref: &str,
    user_id: &str,
    role: &str,
    output: &Output,
) -> CliResult<()> {
    let user_uuid = user_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid user ID".into()))?;

    let member = client.cloud_members_add(project_ref, user_uuid, role).await?;

    if output.format().is_json() {
        output.success(&member)?;
    } else {
        output.success_message(&format!(
            "Added {} as {} to {}",
            user_id, role, project_ref
        ))?;
    }

    Ok(())
}

async fn remove_member(
    client: &Client,
    project_ref: &str,
    user_id: &str,
    output: &Output,
) -> CliResult<()> {
    let user_uuid = user_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid user ID".into()))?;

    client.cloud_members_remove(project_ref, user_uuid).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "removed": user_id }))?;
    } else {
        output.success_message(&format!("Removed {} from {}", user_id, project_ref))?;
    }

    Ok(())
}

// ============================================================================
// Audit
// ============================================================================

async fn get_audit(
    client: &Client,
    project_ref: &str,
    limit: i32,
    output: &Output,
) -> CliResult<()> {
    let entries = client
        .cloud_audit_project(project_ref, Some(limit), None)
        .await?;

    if output.format().is_json() {
        output.success(&entries)?;
    } else if entries.is_empty() {
        output.info("No audit entries found.");
    } else {
        let headers = &["ID", "ACTION", "ACTOR", "TIMESTAMP"];
        let rows: Vec<Vec<String>> = entries
            .iter()
            .map(|e| {
                vec![
                    e.id.to_string(),
                    e.action.clone(),
                    e.actor.clone(),
                    e.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}
