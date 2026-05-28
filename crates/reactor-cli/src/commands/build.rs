//! Build command implementation.

use crate::bundle::{build_bundle, DEFAULT_BUNDLE_NAME};
use crate::cli::{Cli, BuildArgs};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;

pub async fn run(cli: &Cli, args: &BuildArgs, output: &Output) -> CliResult<()> {
    let cwd = std::env::current_dir()?;
    let project = Project::resolve(&cwd, cli.manifest.as_deref())?;

    // Determine output path
    let out_path = args
        .out
        .clone()
        .unwrap_or_else(|| project.root.join(DEFAULT_BUNDLE_NAME));

    output.info(&format!("Building bundle for project '{}'...", project.manifest.name));

    // Build the bundle
    let manifest = build_bundle(&project, &out_path)?;

    // Report what was included
    let mut included = Vec::new();
    if let Some(ref data) = manifest.capabilities.data {
        included.push(format!("{} migration(s)", data.migrations.len()));
    }
    if let Some(ref functions) = manifest.capabilities.functions {
        included.push(format!("{} function(s)", functions.len()));
    }
    if let Some(ref sites) = manifest.capabilities.sites {
        included.push(format!("{} site(s)", sites.len()));
    }
    if let Some(ref jobs) = manifest.capabilities.jobs {
        included.push(format!("{} job(s)", jobs.len()));
    }

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "project_id": manifest.project_id,
            "output": out_path.display().to_string(),
            "capabilities": {
                "data": manifest.capabilities.data.map(|d| d.migrations.len()),
                "functions": manifest.capabilities.functions.as_ref().map(|f| f.len()),
                "sites": manifest.capabilities.sites.as_ref().map(|s| s.len()),
                "jobs": manifest.capabilities.jobs.as_ref().map(|j| j.len()),
            }
        }))?;
    } else {
        output.info(&format!("  Included: {}", included.join(", ")));
        output.success_message(&format!("Bundle created: {}", out_path.display()))?;
    }

    Ok(())
}
