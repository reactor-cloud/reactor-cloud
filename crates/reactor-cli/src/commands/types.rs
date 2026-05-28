//! Types generation command.

use crate::cli::{Cli, TypesArgs, TypesCommands};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};
use std::path::PathBuf;

/// Run a types command.
pub async fn run(cli: &Cli, args: &TypesArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match &args.command {
        TypesCommands::Generate { output: output_path } => {
            generate(&client, output_path, output).await
        }
    }
}

fn build_client(cli: &Cli) -> CliResult<Client> {
    let config = GlobalConfig::load()?;
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

    let mut client_config = ClientConfig::new(resolved.endpoint);
    if let Some(token) = resolved.token {
        client_config = client_config.with_token(token);
    }
    if let Some(org) = resolved.org {
        client_config = client_config.with_org(org);
    }

    Client::new(client_config).map_err(Into::into)
}

/// Generate TypeScript types from the database schema.
async fn generate(client: &Client, output_path: &PathBuf, output: &Output) -> CliResult<()> {
    // Fetch the types from the server
    let types_content = client.get_text("/data/v1/_admin/types/typescript").await?;

    // Write to the output file
    std::fs::write(output_path, &types_content)?;

    let bytes_written = types_content.len();

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "written": output_path.display().to_string(),
            "bytes": bytes_written,
        }))?;
    } else {
        output.success_message(&format!(
            "Generated types to {} ({} bytes)",
            output_path.display(),
            bytes_written
        ))?;
    }

    Ok(())
}
