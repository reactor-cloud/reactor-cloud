//! Project command implementation.

use crate::cli::{Cli, ProjectArgs, ProjectCommands};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;

pub async fn run(cli: &Cli, args: &ProjectArgs, output: &Output) -> CliResult<()> {
    match &args.command {
        ProjectCommands::Show => show(cli, output).await,
    }
}

async fn show(cli: &Cli, output: &Output) -> CliResult<()> {
    let cwd = std::env::current_dir()?;
    let project = Project::resolve(&cwd, cli.manifest.as_deref())?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "project_id": project.manifest.project_id,
            "name": project.manifest.name,
            "default_context": project.manifest.default_context,
            "manifest_path": project.manifest_path.display().to_string(),
            "root": project.root.display().to_string(),
            "functions": project.manifest.functions.iter().map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "source": f.source,
                    "runtime": f.runtime,
                })
            }).collect::<Vec<_>>(),
            "sites": project.manifest.sites.iter().map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "source": s.source,
                    "framework": s.framework,
                })
            }).collect::<Vec<_>>(),
            "jobs": project.manifest.jobs.iter().map(|j| {
                serde_json::json!({
                    "name": j.name,
                    "function": j.function,
                })
            }).collect::<Vec<_>>(),
            "data": project.manifest.data.as_ref().map(|d| {
                serde_json::json!({
                    "migrations_dir": d.migrations_dir,
                })
            }),
        }))?;
    } else {
        use crate::output::human;

        human::print_section("Project");
        human::print_kv("Name", &project.manifest.name);
        human::print_kv("ID", &project.manifest.project_id);
        human::print_kv("Root", &project.root.display().to_string());
        human::print_kv("Manifest", &project.manifest_path.display().to_string());

        if let Some(ctx) = &project.manifest.default_context {
            human::print_kv("Default Context", ctx);
        }

        if !project.manifest.functions.is_empty() {
            human::print_section("Functions");
            for f in &project.manifest.functions {
                human::print_bullet(&format!("{} ({}) - {}", f.name, f.runtime, f.source));
            }
        }

        if !project.manifest.sites.is_empty() {
            human::print_section("Sites");
            for s in &project.manifest.sites {
                human::print_bullet(&format!("{} ({}) - {}", s.name, s.framework, s.source));
            }
        }

        if !project.manifest.jobs.is_empty() {
            human::print_section("Jobs");
            for j in &project.manifest.jobs {
                human::print_bullet(&format!("{} -> {}", j.name, j.function));
            }
        }

        if let Some(data) = &project.manifest.data {
            human::print_section("Data");
            human::print_kv("Migrations", &data.migrations_dir);
        }
    }

    Ok(())
}
