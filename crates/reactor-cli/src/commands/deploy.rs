//! Deploy command implementation.

use crate::bundle::{build_bundle, read_bundle, DEFAULT_BUNDLE_NAME};
use crate::cli::{Cli, DeployArgs};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use reactor_client::admin::DeployStatus;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, args: &DeployArgs, output: &Output) -> CliResult<()> {
    let config = GlobalConfig::load()?;

    // Resolve project
    let cwd = std::env::current_dir()?;
    let project = Project::resolve(&cwd, cli.manifest.as_deref())?;
    let project_default = project.manifest.default_context.as_deref();

    // Resolve context
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

    // Get bundle bytes
    let bundle_data = if let Some(bundle_path) = &args.bundle {
        output.info(&format!("Using pre-built bundle: {}", bundle_path.display()));
        read_bundle(bundle_path)?
    } else if args.prebuilt {
        let bundle_path = project.root.join(DEFAULT_BUNDLE_NAME);
        if !bundle_path.exists() {
            return Err(CliError::BundleValidation(format!(
                "Pre-built bundle not found at {}. Run 'reactor build' first.",
                bundle_path.display()
            )));
        }
        output.info(&format!("Using pre-built bundle: {}", bundle_path.display()));
        read_bundle(&bundle_path)?
    } else {
        // Build bundle
        output.info(&format!("Building bundle for '{}'...", project.manifest.name));
        let bundle_path = project.root.join(DEFAULT_BUNDLE_NAME);
        build_bundle(&project, &bundle_path)?;
        read_bundle(&bundle_path)?
    };

    output.info(&format!(
        "Deploying to {} ({})...",
        resolved.name, resolved.endpoint
    ));

    // Deploy
    let result = client.deploy(bundle_data).await?;

    // Report result
    if output.format().is_json() {
        output.success(&serde_json::json!({
            "deploy_id": result.deploy_id,
            "status": format!("{:?}", result.status).to_lowercase(),
            "phases": result.phases.iter().map(|p| {
                serde_json::json!({
                    "capability": p.capability,
                    "status": p.status,
                    "details": p.details,
                })
            }).collect::<Vec<_>>()
        }))?;
    } else {
        output.info(&format!("Deploy ID: {}", result.deploy_id));

        for phase in &result.phases {
            let status_symbol = if phase.status == "ok" { "✓" } else { "✗" };
            output.info(&format!("  {} {}: {}", status_symbol, phase.capability, phase.status));
        }

        match result.status {
            DeployStatus::Ok => {
                output.success_message("Deployment successful!")?;
            }
            DeployStatus::Partial => {
                output.warning("Deployment partially succeeded. Some capabilities failed.");
            }
            DeployStatus::Failed => {
                return Err(CliError::DeploymentFailed(
                    "All capabilities failed".to_string(),
                ));
            }
        }
    }

    // Return non-zero exit for partial/failed
    match result.status {
        DeployStatus::Ok => Ok(()),
        DeployStatus::Partial => Err(CliError::PartialDeployment {
            succeeded: result.phases.iter().filter(|p| p.status == "ok").count(),
            failed: result.phases.iter().filter(|p| p.status != "ok").count(),
        }),
        DeployStatus::Failed => Err(CliError::DeploymentFailed(
            "Deployment failed".to_string(),
        )),
    }
}
