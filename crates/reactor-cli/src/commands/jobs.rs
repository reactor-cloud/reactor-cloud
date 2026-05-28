//! Jobs command implementation.

use crate::cli::{Cli, JobsArgs, JobsCommands, JobsDlqCommands};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, args: &JobsArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match &args.command {
        JobsCommands::List => list(&client, output).await,
        JobsCommands::Show { name } => show(&client, name, output).await,
        JobsCommands::Trigger { name, data } => trigger(&client, name, data.as_deref(), output).await,
        JobsCommands::Runs { name, limit } => runs(&client, name, *limit, output).await,
        JobsCommands::Run { run_id } => run_show(&client, run_id, output).await,
        JobsCommands::Dlq(dlq_args) => match &dlq_args.command {
            JobsDlqCommands::List { job } => dlq_list(&client, job.as_deref(), output).await,
            JobsDlqCommands::Replay { id } => dlq_replay(&client, id, output).await,
            JobsDlqCommands::Purge { job } => dlq_purge(&client, job.as_deref(), output).await,
        },
        JobsCommands::Logs { name, since } => logs(&client, name, since.as_deref(), output).await,
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

async fn list(client: &Client, output: &Output) -> CliResult<()> {
    let jobs = client.jobs_list().await?;

    if output.format().is_json() {
        output.success(&jobs)?;
    } else if jobs.is_empty() {
        output.info("No jobs found.");
    } else {
        let headers = &["NAME", "FUNCTION", "TRIGGER", "STATUS"];
        let rows: Vec<Vec<String>> = jobs
            .iter()
            .map(|j| {
                let trigger = match &j.trigger {
                    reactor_client::jobs::JobTrigger::Cron { schedule } => format!("cron: {}", schedule),
                    reactor_client::jobs::JobTrigger::Event { event_type } => format!("event: {}", event_type),
                    reactor_client::jobs::JobTrigger::Manual => "manual".to_string(),
                };
                vec![j.name.clone(), j.function_name.clone(), trigger, j.status.clone()]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn show(client: &Client, name: &str, output: &Output) -> CliResult<()> {
    let job = client.jobs_get(name).await?;

    if output.format().is_json() {
        output.success(&job)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Job: {}", job.name));
        human::print_kv("ID", &job.id.to_string());
        human::print_kv("Function", &job.function_name);
        human::print_kv("Status", &job.status);
        human::print_kv("Created", &job.created_at.to_rfc3339());
        human::print_kv("Updated", &job.updated_at.to_rfc3339());
    }

    Ok(())
}

async fn trigger(client: &Client, name: &str, data: Option<&str>, output: &Output) -> CliResult<()> {
    let payload = if let Some(d) = data {
        if d.starts_with('@') {
            let path = &d[1..];
            let content = std::fs::read_to_string(path)?;
            serde_json::from_str(&content)?
        } else {
            serde_json::from_str(d)?
        }
    } else {
        None
    };

    let run = client.jobs_trigger(name, payload).await?;

    if output.format().is_json() {
        output.success(&run)?;
    } else {
        output.success_message(&format!("Job '{}' triggered. Run ID: {}", name, run.id))?;
    }

    Ok(())
}

async fn runs(client: &Client, name: &str, limit: Option<u32>, output: &Output) -> CliResult<()> {
    let runs = client.jobs_runs_list(name, limit).await?;

    if output.format().is_json() {
        output.success(&runs)?;
    } else if runs.is_empty() {
        output.info("No runs found.");
    } else {
        let headers = &["ID", "STATUS", "STARTED", "COMPLETED"];
        let rows: Vec<Vec<String>> = runs
            .iter()
            .map(|r| {
                vec![
                    r.id.to_string(),
                    format!("{:?}", r.status).to_lowercase(),
                    r.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    r.completed_at
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn run_show(client: &Client, run_id: &str, output: &Output) -> CliResult<()> {
    let id = run_id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid run ID".into()))?;
    let run = client.jobs_run_get(id).await?;

    if output.format().is_json() {
        output.success(&run)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Run: {}", run.id));
        human::print_kv("Job ID", &run.job_id.to_string());
        human::print_kv("Status", &format!("{:?}", run.status).to_lowercase());
        human::print_kv("Started", &run.started_at.to_rfc3339());
        if let Some(completed) = run.completed_at {
            human::print_kv("Completed", &completed.to_rfc3339());
        }
        if let Some(error) = &run.error {
            human::print_kv("Error", error);
        }
    }

    Ok(())
}

async fn dlq_list(client: &Client, job: Option<&str>, output: &Output) -> CliResult<()> {
    let entries = client.jobs_dlq_list(job).await?;

    if output.format().is_json() {
        output.success(&entries)?;
    } else if entries.is_empty() {
        output.info("No DLQ entries found.");
    } else {
        let headers = &["ID", "JOB_ID", "ERROR", "ATTEMPTS", "CREATED"];
        let rows: Vec<Vec<String>> = entries
            .iter()
            .map(|e| {
                vec![
                    e.id.to_string(),
                    e.job_id.to_string(),
                    e.error.chars().take(50).collect::<String>() + "...",
                    e.attempts.to_string(),
                    e.created_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn dlq_replay(client: &Client, id: &str, output: &Output) -> CliResult<()> {
    let dlq_id = id
        .parse()
        .map_err(|_| CliError::InvalidArgument("invalid DLQ entry ID".into()))?;
    let run = client.jobs_dlq_replay(dlq_id).await?;

    if output.format().is_json() {
        output.success(&run)?;
    } else {
        output.success_message(&format!("DLQ entry replayed. New run ID: {}", run.id))?;
    }

    Ok(())
}

async fn dlq_purge(client: &Client, job: Option<&str>, output: &Output) -> CliResult<()> {
    let count = client.jobs_dlq_purge(job).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "purged": count }))?;
    } else {
        output.success_message(&format!("Purged {} DLQ entries.", count))?;
    }

    Ok(())
}

async fn logs(client: &Client, name: &str, since: Option<&str>, output: &Output) -> CliResult<()> {
    let logs = client.jobs_logs(name, since, Some(100)).await?;

    if output.format().is_json() {
        output.success(&logs)?;
    } else if logs.is_empty() {
        output.info("No logs found.");
    } else {
        for entry in logs {
            println!(
                "{} [{}] {}",
                entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                entry.level,
                entry.message
            );
        }
    }

    Ok(())
}
