//! Vault secret management commands.

use crate::cli::{Cli, VaultCommands};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::CliResult;
use crate::output::Output;
use crate::project::Project;
use reactor_client::{Client, ClientConfig};

pub async fn run(cli: &Cli, command: &VaultCommands, output: &Output) -> CliResult<()> {
    let client = build_client(cli)?;

    match command {
        VaultCommands::List => list(&client, output).await,
        VaultCommands::Get { key } => get(&client, key, output).await,
        VaultCommands::Set { key, value } => set(&client, key, value, output).await,
        VaultCommands::Unset { key } => unset(&client, key, output).await,
        VaultCommands::Rotate { key_name } => rotate(&client, key_name, output).await,
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

    let mut client_config = ClientConfig::new(resolved.endpoint.clone());
    if let Some(token) = &resolved.token {
        client_config = client_config.with_token(token);
    }

    Ok(Client::new(client_config)?)
}

async fn list(client: &Client, output: &Output) -> CliResult<()> {
    let result = client.vault_list().await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "secrets": result.secrets }))?;
    } else {
        if result.secrets.is_empty() {
            println!("No secrets found.");
        } else {
            println!("Secrets:");
            for secret in &result.secrets {
                println!("  {} (v{})", secret.name, secret.version);
            }
        }
    }

    Ok(())
}

async fn get(client: &Client, key: &str, output: &Output) -> CliResult<()> {
    let result = client.vault_get(key).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "key": result.key,
            "value": result.value,
            "is_base64": result.is_base64,
            "version": result.version,
        }))?;
    } else {
        if result.is_base64 {
            println!("Key: {}", result.key);
            println!("Value (base64): {}", result.value);
            println!("Version: {}", result.version);
        } else {
            println!("{}", result.value);
        }
    }

    Ok(())
}

async fn set(client: &Client, key: &str, value: &str, output: &Output) -> CliResult<()> {
    // Handle special value formats
    let (actual_value, is_base64) = if value.starts_with('@') {
        // Read from file
        let file_path = &value[1..];
        let contents = std::fs::read_to_string(file_path)?;
        (contents, false)
    } else if value == "-" {
        // Read from stdin
        use std::io::Read;
        let mut contents = String::new();
        std::io::stdin().read_to_string(&mut contents)?;
        (contents.trim_end().to_string(), false)
    } else {
        (value.to_string(), false)
    };

    client.vault_set(key, &actual_value, is_base64).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "status": "ok", "key": key }))?;
    } else {
        output.success(&format!("Secret '{}' set.", key))?;
    }

    Ok(())
}

async fn unset(client: &Client, key: &str, output: &Output) -> CliResult<()> {
    client.vault_delete(key).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "status": "ok", "key": key }))?;
    } else {
        output.success(&format!("Secret '{}' deleted.", key))?;
    }

    Ok(())
}

async fn rotate(client: &Client, key_name: &str, output: &Output) -> CliResult<()> {
    let result = client.vault_rotate(key_name).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "status": "ok",
            "key_name": key_name,
            "new_version": result.new_version,
        }))?;
    } else {
        output.success(&format!(
            "Key '{}' rotated to version {}.",
            key_name, result.new_version
        ))?;
    }

    Ok(())
}
