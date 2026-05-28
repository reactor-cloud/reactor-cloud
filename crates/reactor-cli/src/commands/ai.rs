//! AI command implementation.

use crate::cli::{AiArgs, AiCommands, Cli};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, args: &AiArgs, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match &args.command {
        AiCommands::Models(models_args) => models(&client, models_args.capability.as_deref(), output).await,
        AiCommands::Aliases(_) => aliases(&client, output).await,
        AiCommands::Test(test_args) => test(&client, test_args, output, cli.verbose).await,
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

async fn models(client: &Client, capability: Option<&str>, output: &Output) -> CliResult<()> {
    let response = client.ai_models_list().await?;

    if output.format().is_json() {
        output.success(&response)?;
    } else {
        let filtered: Vec<_> = response.data.iter()
            .filter(|m| capability.map_or(true, |c| m.id.contains(c)))
            .collect();

        if filtered.is_empty() {
            output.info("No models found.");
        } else {
            let headers = &["ID", "PROVIDER", "CREATED"];
            let rows: Vec<Vec<String>> = filtered.iter()
                .map(|m| vec![
                    m.id.clone(),
                    m.owned_by.clone(),
                    m.created.to_string(),
                ])
                .collect();
            output.table(headers, rows)?;
        }
    }

    Ok(())
}

async fn aliases(client: &Client, output: &Output) -> CliResult<()> {
    // Note: The API doesn't have a dedicated aliases endpoint yet.
    // For now, we can list models that look like aliases.
    let response = client.ai_models_list().await?;

    if output.format().is_json() {
        output.success(&response)?;
    } else {
        let aliases: Vec<_> = response.data.iter()
            .filter(|m| m.id.contains('/') && (
                m.id.contains("cheapest") || 
                m.id.contains("fastest") || 
                m.id.contains("latest") ||
                m.id.contains("best")
            ))
            .collect();

        if aliases.is_empty() {
            output.info("No aliases found.");
        } else {
            let headers = &["ALIAS", "RESOLVES TO"];
            let rows: Vec<Vec<String>> = aliases.iter()
                .map(|m| vec![
                    m.id.clone(),
                    m.owned_by.clone(),
                ])
                .collect();
            output.table(headers, rows)?;
        }
    }

    Ok(())
}

async fn test(
    client: &Client,
    args: &crate::cli::AiTestArgs,
    output: &Output,
    verbose: bool,
) -> CliResult<()> {
    use reactor_client::ai::{user_message, system_message, ChatCompletionRequest, Message};

    let prompt = if let Some(ref p) = args.prompt {
        p.clone()
    } else {
        return Err(crate::error::CliError::User("prompt is required (use --prompt)".into()));
    };

    let mut messages: Vec<Message> = Vec::new();
    
    if let Some(ref sys) = args.system {
        messages.push(system_message(sys));
    }
    messages.push(user_message(&prompt));

    let mut request = ChatCompletionRequest::new(&args.model, messages);
    if let Some(max_tokens) = args.max_tokens {
        request = request.with_max_tokens(max_tokens);
    }

    if args.stream {
        // Note: Streaming is not yet fully implemented in the CLI.
        // For now, fall through to non-streaming.
        output.warning("Streaming mode is not yet supported in CLI, using non-streaming.");
    }

    {
        let response = client.ai_chat_completion(request).await?;

        if output.format().is_json() {
            output.success(&response)?;
        } else {
            if verbose {
                output.info(&format!("Model: {}", response.model));
                output.info(&format!("ID: {}", response.id));
            }

            for choice in &response.choices {
                if let Some(ref content) = choice.message.content {
                    output.info("Response:");
                    println!("{}", content);
                }
            }

            if let Some(ref usage) = response.usage {
                output.info(&format!(
                    "Tokens: {} in / {} out / {} total",
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens
                ));
            }
        }
    }

    Ok(())
}
