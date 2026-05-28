//! Context command implementation.

use crate::cli::{Cli, ContextArgs, ContextCommands};
use crate::context::{AuthConfig, ContextConfig, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;

pub async fn run(_cli: &Cli, args: &ContextArgs, output: &Output) -> CliResult<()> {
    match &args.command {
        ContextCommands::List => list(output).await,
        ContextCommands::Add {
            name,
            endpoint,
            org,
            token_env,
            token,
        } => add(name, endpoint, org.as_deref(), token_env.as_deref(), token.as_deref(), output).await,
        ContextCommands::Use { name } => set_default(name, output).await,
        ContextCommands::Show { name } => show(name.as_deref(), output).await,
        ContextCommands::Remove { name } => remove(name, output).await,
    }
}

async fn list(output: &Output) -> CliResult<()> {
    let config = GlobalConfig::load()?;

    if config.contexts.is_empty() {
        if output.format().is_json() {
            output.success(&serde_json::json!({
                "default": config.default,
                "contexts": []
            }))?;
        } else {
            output.info("No contexts configured.");
            output.info("Run 'reactor context add <name> --endpoint <url>' to add one.");
        }
        return Ok(());
    }

    if output.format().is_json() {
        let contexts: Vec<_> = config
            .contexts
            .iter()
            .map(|(name, ctx)| {
                serde_json::json!({
                    "name": name,
                    "endpoint": ctx.endpoint,
                    "org": ctx.org,
                    "is_default": config.default.as_ref() == Some(name),
                })
            })
            .collect();

        output.success(&serde_json::json!({
            "default": config.default,
            "contexts": contexts
        }))?;
    } else {
        let headers = &["NAME", "ENDPOINT", "ORG", "DEFAULT"];
        let rows: Vec<Vec<String>> = config
            .contexts
            .iter()
            .map(|(name, ctx)| {
                vec![
                    name.clone(),
                    ctx.endpoint.clone(),
                    ctx.org.clone().unwrap_or_default(),
                    if config.default.as_ref() == Some(name) {
                        "✓".to_string()
                    } else {
                        "".to_string()
                    },
                ]
            })
            .collect();

        output.table(headers, rows)?;
    }

    Ok(())
}

async fn add(
    name: &str,
    endpoint: &str,
    org: Option<&str>,
    token_env: Option<&str>,
    token: Option<&str>,
    output: &Output,
) -> CliResult<()> {
    // Validate endpoint is a valid URL
    url::Url::parse(endpoint)?;

    let mut config = GlobalConfig::load()?;

    // Check if context already exists
    if config.contexts.contains_key(name) {
        return Err(CliError::User(format!(
            "Context '{}' already exists. Use 'reactor context remove {}' first.",
            name, name
        )));
    }

    // Build context config
    let mut ctx = ContextConfig::new(endpoint);

    if let Some(o) = org {
        ctx = ctx.with_org(o);
    }

    // Handle auth configuration
    if let Some(env) = token_env {
        ctx.auth = AuthConfig::TokenEnv { env: env.to_string() };
    } else if let Some(t) = token {
        // Store token in keychain
        #[cfg(feature = "keyring")]
        {
            crate::context::store_token_keychain(name, t)?;
            ctx.auth = AuthConfig::Keychain {
                service: "reactor".to_string(),
                account: name.to_string(),
            };
        }
        #[cfg(not(feature = "keyring"))]
        {
            // Fall back to token-env approach with a warning
            let env_name = format!("REACTOR_{}_TOKEN", name.to_uppercase().replace('-', "_"));
            output.warning(&format!(
                "Keyring not available. Set {} environment variable instead.",
                env_name
            ));
            ctx.auth = AuthConfig::TokenEnv { env: env_name };
            let _ = t; // Suppress unused warning
        }
    }

    // Add context
    config.set_context(name.to_string(), ctx.clone());

    // Set as default if first context
    if config.contexts.len() == 1 {
        config.set_default(Some(name.to_string()));
    }

    config.save()?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "name": name,
            "endpoint": ctx.endpoint,
            "org": ctx.org,
            "is_default": config.default.as_ref() == Some(&name.to_string()),
        }))?;
    } else {
        output.success_message(&format!("Context '{}' added.", name))?;
        if config.default.as_ref() == Some(&name.to_string()) {
            output.info(&format!("Set as default context."));
        }
    }

    Ok(())
}

async fn set_default(name: &str, output: &Output) -> CliResult<()> {
    let mut config = GlobalConfig::load()?;

    // Check if context exists
    if !config.contexts.contains_key(name) {
        return Err(CliError::ContextNotFound(name.to_string()));
    }

    config.set_default(Some(name.to_string()));
    config.save()?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "default": name
        }))?;
    } else {
        output.success_message(&format!("Default context set to '{}'.", name))?;
    }

    Ok(())
}

async fn show(name: Option<&str>, output: &Output) -> CliResult<()> {
    let config = GlobalConfig::load()?;

    // Determine which context to show
    let name = name
        .or(config.default.as_deref())
        .ok_or_else(|| CliError::Config("no context specified and no default set".into()))?;

    let ctx = config
        .get_context(name)
        .ok_or_else(|| CliError::ContextNotFound(name.to_string()))?;

    let auth_type = match &ctx.auth {
        AuthConfig::None => "none",
        AuthConfig::TokenEnv { .. } => "token-env",
        AuthConfig::Keychain { .. } => "keychain",
        AuthConfig::Session { .. } => "session",
    };

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "name": name,
            "endpoint": ctx.endpoint,
            "org": ctx.org,
            "auth": {
                "type": auth_type,
            },
            "is_default": config.default.as_ref() == Some(&name.to_string()),
        }))?;
    } else {
        use crate::output::human;

        human::print_section(&format!("Context: {}", name));
        human::print_kv("Endpoint", &ctx.endpoint);
        if let Some(org) = &ctx.org {
            human::print_kv("Organization", org);
        }
        human::print_kv("Auth", auth_type);
        if config.default.as_ref() == Some(&name.to_string()) {
            human::print_kv("Default", "yes");
        }
    }

    Ok(())
}

async fn remove(name: &str, output: &Output) -> CliResult<()> {
    let mut config = GlobalConfig::load()?;

    // Check if context exists
    let ctx = config.remove_context(name);
    if ctx.is_none() {
        return Err(CliError::ContextNotFound(name.to_string()));
    }

    // Remove from keychain if applicable
    #[cfg(feature = "keyring")]
    if let Some(ContextConfig { auth: AuthConfig::Keychain { .. }, .. }) = &ctx {
        let _ = crate::context::remove_token_keychain(name);
    }

    // Clear default if this was the default
    if config.default.as_ref() == Some(&name.to_string()) {
        config.set_default(None);
    }

    config.save()?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "removed": name
        }))?;
    } else {
        output.success_message(&format!("Context '{}' removed.", name))?;
    }

    Ok(())
}
