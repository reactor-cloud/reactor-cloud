//! Migrate command implementation.

use crate::cli::{Cli, MigrateArgs};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, args: &MigrateArgs, output: &Output) -> CliResult<()> {
    let config = GlobalConfig::load()?;

    // Resolve project and context
    let cwd = std::env::current_dir()?;
    let project = Project::try_resolve(&cwd, cli.manifest.as_deref());
    let project_default = project
        .as_ref()
        .and_then(|p| p.manifest.default_context.as_deref());

    let resolved = resolve_context(
        &config,
        cli.context.as_deref(),
        project_default,
        cli.token.as_deref(),
    )?;

    // Build client
    let mut client_config = ClientConfig::new(resolved.endpoint.clone());
    if let Some(token) = &resolved.token {
        client_config = client_config.with_token(token);
    }
    if let Some(org) = &resolved.org {
        client_config = client_config.with_org(org);
    }

    let client = Client::new(client_config)?;

    if args.dry_run {
        output.info("Running migrations (dry run)...");
    } else {
        output.info("Running migrations...");
    }

    // Call migrate endpoint
    let result = client.migrate(args.dry_run).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "dry_run": args.dry_run,
            "applied": result.applied,
            "skipped": result.skipped,
        }))?;
    } else {
        if result.applied.is_empty() && result.skipped.is_empty() {
            output.success_message("No migrations to apply.")?;
        } else {
            if !result.applied.is_empty() {
                if args.dry_run {
                    output.info("Would apply:");
                } else {
                    output.info("Applied:");
                }
                for migration in &result.applied {
                    output.info(&format!("  - {}", migration));
                }
            }

            if !result.skipped.is_empty() {
                output.info("Skipped (already applied):");
                for migration in &result.skipped {
                    output.info(&format!("  - {}", migration));
                }
            }

            if args.dry_run {
                output.success_message(&format!(
                    "Dry run complete. {} migration(s) would be applied.",
                    result.applied.len()
                ))?;
            } else {
                output.success_message(&format!(
                    "Migrations complete. {} applied, {} skipped.",
                    result.applied.len(),
                    result.skipped.len()
                ))?;
            }
        }
    }

    Ok(())
}
