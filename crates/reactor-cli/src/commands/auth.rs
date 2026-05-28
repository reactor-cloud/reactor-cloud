//! Auth admin command implementation.

use crate::cli::{
    AuthArgs, AuthCommands, AuthInvitationsCommands, AuthKeysCommands, AuthMembersCommands,
    AuthOrgsCommands, Cli,
};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use crate::confirm;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, args: &AuthArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match &args.command {
        AuthCommands::Orgs(orgs_args) => match &orgs_args.command {
            AuthOrgsCommands::List => orgs_list(&client, output).await,
            AuthOrgsCommands::Create { name, slug } => orgs_create(&client, name, slug, output).await,
            AuthOrgsCommands::Show { org_id } => orgs_show(&client, org_id, output).await,
            AuthOrgsCommands::Update {
                org_id,
                name,
                slug,
            } => orgs_update(&client, org_id, name.as_deref(), slug.as_deref(), output).await,
            AuthOrgsCommands::Delete { org_id } => orgs_delete(cli, &client, org_id, output).await,
        },
        AuthCommands::Members(members_args) => match &members_args.command {
            AuthMembersCommands::List { org_id } => members_list(&client, org_id, output).await,
            AuthMembersCommands::Add {
                org_id,
                user_id,
                role,
            } => members_add(&client, org_id, user_id, role, output).await,
            AuthMembersCommands::Update {
                org_id,
                user_id,
                role,
            } => members_update(&client, org_id, user_id, role, output).await,
            AuthMembersCommands::Remove { org_id, user_id } => {
                members_remove(cli, &client, org_id, user_id, output).await
            }
        },
        AuthCommands::Keys(keys_args) => match &keys_args.command {
            AuthKeysCommands::List { org_id } => keys_list(&client, org_id, output).await,
            AuthKeysCommands::Create { org_id, name } => keys_create(&client, org_id, name, output).await,
            AuthKeysCommands::Revoke { key_id } => keys_revoke(cli, &client, key_id, output).await,
        },
        AuthCommands::Invitations(invitations_args) => match &invitations_args.command {
            AuthInvitationsCommands::List { org_id } => invitations_list(&client, org_id, output).await,
            AuthInvitationsCommands::Create {
                org_id,
                email,
                role,
            } => invitations_create(&client, org_id, email, role, output).await,
            AuthInvitationsCommands::Revoke { invitation_id } => {
                invitations_revoke(cli, &client, invitation_id, output).await
            }
        },
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

// Organizations
async fn orgs_list(client: &Client, output: &Output) -> CliResult<()> {
    let orgs = client.auth_orgs_list().await?;

    if output.format().is_json() {
        output.success(&orgs)?;
    } else if orgs.is_empty() {
        output.info("No organizations found.");
    } else {
        let headers = &["ID", "NAME", "SLUG", "CREATED"];
        let rows: Vec<Vec<String>> = orgs
            .iter()
            .map(|o| {
                vec![
                    o.id.to_string(),
                    o.name.clone(),
                    o.slug.clone(),
                    o.created_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn orgs_create(client: &Client, name: &str, slug: &str, output: &Output) -> CliResult<()> {
    let org = client.auth_org_create(name, slug).await?;

    if output.format().is_json() {
        output.success(&org)?;
    } else {
        output.success_message(&format!("Created organization '{}' ({})", org.name, org.id))?;
    }

    Ok(())
}

async fn orgs_show(client: &Client, org_id: &str, output: &Output) -> CliResult<()> {
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

async fn orgs_update(
    client: &Client,
    org_id: &str,
    name: Option<&str>,
    slug: Option<&str>,
    output: &Output,
) -> CliResult<()> {
    let id = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let org = client.auth_org_update(id, name, slug).await?;

    if output.format().is_json() {
        output.success(&org)?;
    } else {
        output.success_message("Organization updated.")?;
    }

    Ok(())
}

async fn orgs_delete(cli: &Cli, client: &Client, org_id: &str, output: &Output) -> CliResult<()> {
    let id = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;

    confirm(cli, &format!("Delete organization {}? This cannot be undone.", org_id))?;

    client.auth_org_delete(id).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "deleted": org_id }))?;
    } else {
        output.success_message("Organization deleted.")?;
    }

    Ok(())
}

// Members
async fn members_list(client: &Client, org_id: &str, output: &Output) -> CliResult<()> {
    let id = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let members = client.auth_members_list(id).await?;

    if output.format().is_json() {
        output.success(&members)?;
    } else if members.is_empty() {
        output.info("No members found.");
    } else {
        let headers = &["USER_ID", "EMAIL", "ROLE", "JOINED"];
        let rows: Vec<Vec<String>> = members
            .iter()
            .map(|m| {
                vec![
                    m.user_id.to_string(),
                    m.email.clone().unwrap_or_default(),
                    m.role.clone(),
                    m.joined_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn members_add(
    client: &Client,
    org_id: &str,
    user_id: &str,
    role: &str,
    output: &Output,
) -> CliResult<()> {
    let oid = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let uid = user_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid user ID".into()))?;
    let member = client.auth_member_add(oid, uid, role).await?;

    if output.format().is_json() {
        output.success(&member)?;
    } else {
        output.success_message("Member added.")?;
    }

    Ok(())
}

async fn members_update(
    client: &Client,
    org_id: &str,
    user_id: &str,
    role: &str,
    output: &Output,
) -> CliResult<()> {
    let oid = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let uid = user_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid user ID".into()))?;
    let member = client.auth_member_update(oid, uid, role).await?;

    if output.format().is_json() {
        output.success(&member)?;
    } else {
        output.success_message("Member role updated.")?;
    }

    Ok(())
}

async fn members_remove(
    cli: &Cli,
    client: &Client,
    org_id: &str,
    user_id: &str,
    output: &Output,
) -> CliResult<()> {
    let oid = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let uid = user_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid user ID".into()))?;

    confirm(cli, "Remove this member from the organization?")?;

    client.auth_member_remove(oid, uid).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "removed": user_id }))?;
    } else {
        output.success_message("Member removed.")?;
    }

    Ok(())
}

// API Keys
async fn keys_list(client: &Client, org_id: &str, output: &Output) -> CliResult<()> {
    let id = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let keys = client.auth_keys_list(id).await?;

    if output.format().is_json() {
        output.success(&keys)?;
    } else if keys.is_empty() {
        output.info("No API keys found.");
    } else {
        let headers = &["ID", "NAME", "PREFIX", "CREATED", "LAST_USED"];
        let rows: Vec<Vec<String>> = keys
            .iter()
            .map(|k| {
                vec![
                    k.id.to_string(),
                    k.name.clone(),
                    k.key_prefix.clone(),
                    k.created_at.format("%Y-%m-%d %H:%M").to_string(),
                    k.last_used_at
                        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn keys_create(client: &Client, org_id: &str, name: &str, output: &Output) -> CliResult<()> {
    let id = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let key = client.auth_key_create(id, name).await?;

    if output.format().is_json() {
        output.success(&key)?;
    } else {
        use crate::output::human;
        human::print_section("API Key Created");
        human::print_warning("Store this key securely — you won't be able to see it again!");
        human::print_kv("Key ID", &key.id.to_string());
        human::print_kv("Secret", &key.secret);
    }

    Ok(())
}

async fn keys_revoke(cli: &Cli, client: &Client, key_id: &str, output: &Output) -> CliResult<()> {
    let id = key_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid key ID".into()))?;

    confirm(cli, "Revoke this API key? This cannot be undone.")?;

    client.auth_key_revoke(id).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "revoked": key_id }))?;
    } else {
        output.success_message("API key revoked.")?;
    }

    Ok(())
}

// Invitations
async fn invitations_list(client: &Client, org_id: &str, output: &Output) -> CliResult<()> {
    let id = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let invitations = client.auth_invitations_list(id).await?;

    if output.format().is_json() {
        output.success(&invitations)?;
    } else if invitations.is_empty() {
        output.info("No pending invitations found.");
    } else {
        let headers = &["ID", "EMAIL", "ROLE", "STATUS", "CREATED", "EXPIRES"];
        let rows: Vec<Vec<String>> = invitations
            .iter()
            .map(|i| {
                vec![
                    i.id.to_string(),
                    i.email.clone(),
                    i.role.clone(),
                    format!("{:?}", i.status).to_lowercase(),
                    i.created_at.format("%Y-%m-%d %H:%M").to_string(),
                    i.expires_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn invitations_create(
    client: &Client,
    org_id: &str,
    email: &str,
    role: &str,
    output: &Output,
) -> CliResult<()> {
    let id = org_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid organization ID".into()))?;
    let invitation = client.auth_invitation_create(id, email, role).await?;

    if output.format().is_json() {
        output.success(&invitation)?;
    } else {
        output.success_message(&format!("Invitation sent to '{}' with role '{}'.", email, role))?;
    }

    Ok(())
}

async fn invitations_revoke(cli: &Cli, client: &Client, invitation_id: &str, output: &Output) -> CliResult<()> {
    let id = invitation_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid invitation ID".into()))?;

    confirm(cli, "Revoke this invitation?")?;

    client.auth_invitation_revoke(id).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "revoked": invitation_id }))?;
    } else {
        output.success_message("Invitation revoked.")?;
    }

    Ok(())
}
