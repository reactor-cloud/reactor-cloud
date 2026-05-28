//! Logout command implementation.

use crate::cli::{Cli, LogoutArgs};
use crate::context::{AuthConfig, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;

pub async fn run(cli: &Cli, args: &LogoutArgs, output: &Output) -> CliResult<()> {
    let mut config = GlobalConfig::load()?;

    // Determine context name
    let context_name = args
        .context
        .as_deref()
        .or(cli.context.as_deref())
        .or(config.default.as_deref())
        .ok_or_else(|| CliError::Config("no context specified and no default set".into()))?
        .to_string();

    // Check if context exists
    let ctx = config
        .get_context(&context_name)
        .ok_or_else(|| CliError::ContextNotFound(context_name.clone()))?
        .clone();

    // Remove from keychain if applicable
    #[cfg(feature = "keyring")]
    match &ctx.auth {
        AuthConfig::Keychain { .. } => {
            crate::context::remove_token_keychain(&context_name)?;
        }
        AuthConfig::Session { service, access_account, refresh_account, .. } => {
            // Remove both access and refresh tokens from keychain
            if let Ok(entry) = keyring::Entry::new(service, access_account) {
                let _ = entry.delete_credential();
            }
            if let Ok(entry) = keyring::Entry::new(service, refresh_account) {
                let _ = entry.delete_credential();
            }
        }
        _ => {}
    }

    // Update context to have no auth
    let mut updated_ctx = ctx.clone();
    updated_ctx.auth = AuthConfig::None;
    config.set_context(context_name.clone(), updated_ctx);
    config.save()?;

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "context": context_name,
            "logged_out": true
        }))?;
    } else {
        output.success_message(&format!(
            "Logged out of context '{}'.",
            context_name
        ))?;
    }

    Ok(())
}
