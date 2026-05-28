//! Health checks for reactor-ai-server.

use anyhow::Result;
use reactor_ai::{AiConfig, Registry};

pub async fn run() -> Result<()> {
    println!("Running reactor-ai-server diagnostics...\n");

    // Check config loading
    print!("Configuration... ");
    match AiConfig::from_env() {
        Ok(config) => {
            println!("OK");
            println!("  Bind: {}", config.bind);
            println!("  Deployment: {:?}", config.deployment);
            println!(
                "  OpenRouter: {}",
                if config.has_openrouter() {
                    "configured"
                } else {
                    "not configured"
                }
            );
            println!(
                "  Bedrock: {}",
                if config.has_bedrock() {
                    "configured"
                } else {
                    "not configured"
                }
            );
            println!(
                "  Azure Foundry: {}",
                if config.has_foundry() {
                    "configured"
                } else {
                    "not configured"
                }
            );
        }
        Err(e) => {
            println!("FAILED");
            println!("  Error: {}", e);
        }
    }

    // Check registry loading
    print!("Model registry... ");
    match Registry::load_defaults() {
        Ok(registry) => {
            let model_count = registry.models().count();
            let alias_count = registry.aliases().count();
            println!("OK");
            println!("  Models: {}", model_count);
            println!("  Aliases: {}", alias_count);
        }
        Err(e) => {
            println!("FAILED");
            println!("  Error: {}", e);
        }
    }

    println!("\nDiagnostics complete.");
    Ok(())
}
