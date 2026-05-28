//! Version command implementation.

use crate::cli::Cli;
use crate::context::{resolve_context, GlobalConfig};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, output: &Output) -> CliResult<()> {
    let cli_version = crate::VERSION;

    // Try to get server version
    let server_info = get_server_version(cli).await;

    if output.format().is_json() {
        let mut data = serde_json::json!({
            "cli": {
                "version": cli_version,
            }
        });

        if let Some((ctx_name, version_info)) = &server_info {
            data["server"] = serde_json::json!({
                "context": ctx_name,
                "version": version_info.reactor_server,
                "capabilities": version_info.capabilities,
            });
        }

        output.success(&data)?;
    } else {
        println!("reactor-cli {}", cli_version);

        if let Some((ctx_name, version_info)) = &server_info {
            println!();
            println!("Server ({}):", ctx_name);
            println!("  reactor-server {}", version_info.reactor_server);
            if !version_info.capabilities.is_empty() {
                println!("  capabilities:");
                for (cap, ver) in &version_info.capabilities {
                    println!("    {} {}", cap, ver);
                }
            }

            // Version compatibility check
            let cli_semver = semver::Version::parse(cli_version).ok();
            let server_semver = semver::Version::parse(&version_info.reactor_server).ok();

            if let (Some(cli_v), Some(server_v)) = (cli_semver, server_semver) {
                if cli_v.major != server_v.major {
                    output.warning(&format!(
                        "Major version mismatch! CLI {} vs Server {}",
                        cli_version, version_info.reactor_server
                    ));
                } else if cli_v.minor != server_v.minor {
                    output.info(&format!(
                        "Minor version difference: CLI {} vs Server {}",
                        cli_version, version_info.reactor_server
                    ));
                }
            }
        } else {
            println!();
            println!("Server: not connected");
        }
    }

    Ok(())
}

async fn get_server_version(
    cli: &Cli,
) -> Option<(String, reactor_client::admin::VersionInfo)> {
    let config = GlobalConfig::load().ok()?;

    let cwd = std::env::current_dir().ok()?;
    let project = Project::try_resolve(&cwd, cli.manifest.as_deref());
    let project_default = project
        .as_ref()
        .and_then(|p| p.manifest.default_context.as_deref());

    let resolved = resolve_context(
        &config,
        cli.context.as_deref(),
        project_default,
        cli.token.as_deref(),
    )
    .ok()?;

    let mut client_config = ClientConfig::new(resolved.endpoint.clone());
    if let Some(token) = &resolved.token {
        client_config = client_config.with_token(token);
    }

    let client = Client::new(client_config).ok()?;
    let version_info = client.version().await.ok()?;

    Some((resolved.name, version_info))
}
