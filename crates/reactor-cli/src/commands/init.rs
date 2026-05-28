//! Init command implementation.

use crate::cli::{Cli, InitArgs};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::{
    generate_ignore, generate_manifest, generate_sample_function, generate_sample_migration,
    IGNORE_FILENAME, MANIFEST_FILENAME,
};
use std::fs;
use std::path::Path;

pub async fn run(_cli: &Cli, args: &InitArgs, output: &Output) -> CliResult<()> {
    let cwd = std::env::current_dir()?;
    let project_dir = cwd.join(&args.name);

    // Check if directory already exists
    if project_dir.exists() && !args.force {
        return Err(CliError::User(format!(
            "Directory '{}' already exists. Use --force to overwrite.",
            args.name
        )));
    }

    output.info(&format!("Creating project '{}'...", args.name));

    // Create project directory
    fs::create_dir_all(&project_dir)?;

    // Generate and write manifest
    let manifest = generate_manifest(&args.name);
    let manifest_path = project_dir.join(MANIFEST_FILENAME);
    write_file_if_new(&manifest_path, || {
        toml_edit::ser::to_string_pretty(&manifest)
            .map_err(|e| CliError::InvalidManifest(e.to_string()))
    }, args.force)?;
    output.info(&format!("  Created {}", MANIFEST_FILENAME));

    // Generate and write .reactorignore
    let ignore_path = project_dir.join(IGNORE_FILENAME);
    write_file_if_new(&ignore_path, || Ok(generate_ignore().to_string()), args.force)?;
    output.info(&format!("  Created {}", IGNORE_FILENAME));

    // Create directories
    let dirs = [
        "functions",
        "functions/hello",
        "sites",
        "data",
        "data/migrations",
    ];
    for dir in dirs {
        let dir_path = project_dir.join(dir);
        fs::create_dir_all(&dir_path)?;
    }
    output.info("  Created directories");

    // Create sample function
    let function_path = project_dir.join("functions/hello/index.ts");
    write_file_if_new(&function_path, || Ok(generate_sample_function().to_string()), args.force)?;
    output.info("  Created sample function: functions/hello/index.ts");

    // Create sample migration
    let migration_path = project_dir.join("data/migrations/001_sample.sql");
    write_file_if_new(&migration_path, || Ok(generate_sample_migration().to_string()), args.force)?;
    output.info("  Created sample migration: data/migrations/001_sample.sql");

    // Print success
    if output.format().is_json() {
        output.success(&serde_json::json!({
            "project_id": manifest.project_id,
            "name": manifest.name,
            "path": project_dir.display().to_string(),
            "files": [
                MANIFEST_FILENAME,
                IGNORE_FILENAME,
                "functions/hello/index.ts",
                "data/migrations/001_sample.sql"
            ]
        }))?;
    } else {
        output.success_message(&format!(
            "Project '{}' created successfully!\n\n\
            Next steps:\n\
            \n  cd {}\
            \n  reactor dev        # Start local development server\
            \n  reactor deploy     # Deploy to a server",
            args.name, args.name
        ))?;
    }

    Ok(())
}

/// Write a file if it doesn't exist or if force is true.
fn write_file_if_new<F>(path: &Path, content: F, force: bool) -> CliResult<()>
where
    F: FnOnce() -> CliResult<String>,
{
    if path.exists() && !force {
        return Ok(());
    }
    let content = content()?;
    fs::write(path, content)?;
    Ok(())
}
