//! Reactor Connect Server — standalone binary for connector operations
//!
//! This binary runs the Connect capability as a standalone service.
//! For embedded deployment, use `reactor-server` which composes all capabilities.

use anyhow::Result;
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod doctor;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "reactor_connect=debug,reactor_connect_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Check for subcommands
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "doctor" => return doctor::run().await,
            "version" | "--version" | "-V" => {
                println!("reactor-connect-server {}", reactor_connect::VERSION);
                return Ok(());
            }
            _ => {}
        }
    }

    tracing::info!("Starting Reactor Connect Server v{}", reactor_connect::VERSION);

    // TODO: Load config, build state, run server
    // This will be completed in M1.9 after store and routes are implemented
    
    tracing::warn!("Server implementation pending — run 'doctor' to check dependencies");
    
    Ok(())
}
