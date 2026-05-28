//! Whoami command implementation.

use crate::cli::Cli;
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, output: &Output) -> CliResult<()> {
    let config = GlobalConfig::load()?;

    // Try to get project default context
    let cwd = std::env::current_dir()?;
    let project = Project::try_resolve(&cwd, cli.manifest.as_deref());
    let project_default = project.as_ref().and_then(|p| p.manifest.default_context.as_deref());

    // Resolve context
    let resolved = resolve_context(
        &config,
        cli.context.as_deref(),
        project_default,
        cli.token.as_deref(),
    )?;

    // Check if we have a token
    if resolved.token.is_none() {
        if output.format().is_json() {
            output.success(&serde_json::json!({
                "context": resolved.name,
                "authenticated": false,
                "message": "Not logged in"
            }))?;
        } else {
            output.info(&format!("Context: {}", resolved.name));
            output.info("Not logged in.");
            output.info("Run 'reactor login' to authenticate.");
        }
        return Ok(());
    }

    // Build client
    let mut client_config = ClientConfig::new(resolved.endpoint.clone());
    if let Some(token) = &resolved.token {
        client_config = client_config.with_token(token);
    }
    if let Some(org) = &resolved.org {
        client_config = client_config.with_org(org);
    }

    let client = Client::new(client_config)?;

    // Try to get user info
    match client.auth_me().await {
        Ok(user) => {
            if output.format().is_json() {
                output.success(&serde_json::json!({
                    "context": resolved.name,
                    "authenticated": true,
                    "user": {
                        "id": user.user_id,
                        "email": user.email,
                        "orgs": user.orgs.iter().map(|o| {
                            serde_json::json!({
                                "id": o.org_id,
                                "slug": o.org_slug,
                                "role": o.role,
                            })
                        }).collect::<Vec<_>>()
                    }
                }))?;
            } else {
                use crate::output::human;

                human::print_section("User");
                human::print_kv("Email", &user.email);
                human::print_kv("User ID", &user.user_id.to_string());
                human::print_kv("Context", &resolved.name);

                if !user.orgs.is_empty() {
                    human::print_section("Organizations");
                    for org in &user.orgs {
                        human::print_bullet(&format!(
                            "{} ({}) - {}",
                            org.org_slug, org.org_id, org.role
                        ));
                    }
                }
            }
        }
        Err(e) => {
            // If /auth/v1/me fails, try /_admin/version to see if we're using an admin token
            match client.version().await {
                Ok(version_info) => {
                    if output.format().is_json() {
                        output.success(&serde_json::json!({
                            "context": resolved.name,
                            "authenticated": true,
                            "admin": true,
                            "server_version": version_info.reactor_server
                        }))?;
                    } else {
                        use crate::output::human;

                        human::print_section("Admin Token");
                        human::print_kv("Context", &resolved.name);
                        human::print_kv("Server", &version_info.reactor_server);
                        human::print_kv("Capabilities", &version_info.capabilities.keys().cloned().collect::<Vec<_>>().join(", "));
                    }
                }
                Err(_) => {
                    return Err(CliError::AuthFailed(format!(
                        "Failed to authenticate: {}",
                        e
                    )));
                }
            }
        }
    }

    Ok(())
}
