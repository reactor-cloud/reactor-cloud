//! Doctor command implementation.

use crate::cli::Cli;
use crate::context::{resolve_context, GlobalConfig};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Check {
    name: String,
    status: String,
    details: String,
}

pub async fn run(cli: &Cli, output: &Output) -> CliResult<()> {
    let mut checks = Vec::new();

    // Check 1: Manifest
    let cwd = std::env::current_dir()?;
    let project = Project::try_resolve(&cwd, cli.manifest.as_deref());

    if let Some(ref p) = project {
        checks.push(Check {
            name: "manifest".to_string(),
            status: "ok".to_string(),
            details: format!("Found {}", p.manifest_path.display()),
        });
    } else {
        checks.push(Check {
            name: "manifest".to_string(),
            status: "warn".to_string(),
            details: "No reactor.toml found. Run 'reactor init' to create one.".to_string(),
        });
    }

    // Check 2: Context
    let config = GlobalConfig::load()?;
    let project_default = project
        .as_ref()
        .and_then(|p| p.manifest.default_context.as_deref());

    let resolved = resolve_context(
        &config,
        cli.context.as_deref(),
        project_default,
        cli.token.as_deref(),
    );

    match resolved {
        Ok(ctx) => {
            checks.push(Check {
                name: "context".to_string(),
                status: "ok".to_string(),
                details: format!("Using context '{}' -> {}", ctx.name, ctx.endpoint),
            });

            // Check 3: Server connectivity
            let mut client_config = ClientConfig::new(ctx.endpoint.clone());
            if let Some(token) = &ctx.token {
                client_config = client_config.with_token(token);
            }
            if let Some(org) = &ctx.org {
                client_config = client_config.with_org(org);
            }

            match Client::new(client_config) {
                Ok(client) => {
                    // Check health
                    match client.health().await {
                        Ok(_) => {
                            checks.push(Check {
                                name: "server".to_string(),
                                status: "ok".to_string(),
                                details: "Server is healthy".to_string(),
                            });
                        }
                        Err(e) => {
                            checks.push(Check {
                                name: "server".to_string(),
                                status: "fail".to_string(),
                                details: format!("Server unreachable: {}", e),
                            });
                        }
                    }

                    // Check 4: Server doctor (if server is reachable)
                    match client.doctor().await {
                        Ok(doctor_result) => {
                            for (cap_name, health) in doctor_result.capabilities {
                                checks.push(Check {
                                    name: format!("capability:{}", cap_name),
                                    status: health.status.clone(),
                                    details: if health.status == "ok" {
                                        "Healthy".to_string()
                                    } else {
                                        format!(
                                            "{:?}",
                                            health.details
                                        )
                                    },
                                });
                            }
                        }
                        Err(e) => {
                            checks.push(Check {
                                name: "server_doctor".to_string(),
                                status: "warn".to_string(),
                                details: format!("Could not run server doctor: {}", e),
                            });
                        }
                    }
                }
                Err(e) => {
                    checks.push(Check {
                        name: "client".to_string(),
                        status: "fail".to_string(),
                        details: format!("Failed to create client: {}", e),
                    });
                }
            }
        }
        Err(e) => {
            checks.push(Check {
                name: "context".to_string(),
                status: "warn".to_string(),
                details: format!("No context configured: {}", e),
            });
        }
    }

    // Output results
    let has_failures = checks.iter().any(|c| c.status == "fail");
    let has_warnings = checks.iter().any(|c| c.status == "warn");

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "checks": checks,
            "summary": {
                "total": checks.len(),
                "ok": checks.iter().filter(|c| c.status == "ok").count(),
                "warn": checks.iter().filter(|c| c.status == "warn").count(),
                "fail": checks.iter().filter(|c| c.status == "fail").count(),
            }
        }))?;
    } else {
        use console::style;

        println!();
        for check in &checks {
            let status_symbol = match check.status.as_str() {
                "ok" => style("✓").green(),
                "warn" => style("!").yellow(),
                "fail" => style("✗").red(),
                _ => style("?").dim(),
            };
            println!("{} {}: {}", status_symbol, style(&check.name).bold(), check.details);
        }
        println!();

        if has_failures {
            output.warning("Some checks failed. Review the issues above.");
        } else if has_warnings {
            output.info("All critical checks passed. Some warnings to review.");
        } else {
            output.success_message("All checks passed!")?;
        }
    }

    Ok(())
}
