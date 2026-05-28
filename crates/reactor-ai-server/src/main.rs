//! Reactor AI Server.
//!
//! Boots the reactor-ai HTTP service with:
//! - Configurable auth backend (InProcess or Remote)
//! - Graceful shutdown on SIGTERM/SIGINT
//!
//! Usage:
//!   reactor-ai-server         Start the server
//!   reactor-ai-server doctor  Run health checks
//!   reactor-ai-server --help  Show this help
//!
//! See `docs/reactor-ai.design.md` for configuration options.

mod doctor;

use anyhow::{Context, Result};
use reactor_ai::{router, AiConfig, AiState, Registry};
use reactor_ai::ext::NoopExtensions;
use reactor_core::auth::AuthClient;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "doctor" => return doctor::run().await,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--version" | "-V" => {
                println!("reactor-ai-server {}", reactor_ai::VERSION);
                return Ok(());
            }
            other => {
                eprintln!("Unknown command: {}", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    let config = AiConfig::from_env().context("failed to load config")?;

    init_tracing(&config.log);

    tracing::info!(
        version = reactor_ai::VERSION,
        "starting reactor-ai-server"
    );

    let auth = build_auth_client(&config).await?;
    let registry = Registry::load_defaults().context("failed to load model registry")?;

    let config = Arc::new(config);
    let registry = Arc::new(registry);
    let extensions = Arc::new(NoopExtensions::new());

    let state = AiState::new(config.clone(), registry, auth, extensions);

    // Initialize providers based on config
    #[cfg(feature = "openrouter")]
    let state = if let Some(ref api_key) = config.providers.openrouter_api_key {
        let client = Arc::new(reactor_ai::dispatch::OpenRouterClient::new(api_key.clone()));
        state.with_openrouter(client)
    } else {
        state
    };

    let app = router(state);

    let listener = TcpListener::bind(config.bind)
        .await
        .context("failed to bind")?;

    tracing::info!(bind = %config.bind, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    tracing::info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("received Ctrl+C, starting graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("received SIGTERM, starting graceful shutdown");
        }
    }
}

fn print_help() {
    println!(
        r#"reactor-ai-server {}

USAGE:
    reactor-ai-server [COMMAND]

COMMANDS:
    (none)    Start the HTTP server
    doctor    Run health checks on registry and provider connectivity

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

ENVIRONMENT:
    REACTOR_AI_BIND                    Server bind address (default: 127.0.0.1:8090)
    REACTOR_AI_DEPLOYMENT              'monolith' or 'microservices' (default: monolith)
    REACTOR_AI_PROVIDERS_OPENROUTER_API_KEY    OpenRouter API key
    REACTOR_AI_PROVIDERS_AWS_ACCESS_KEY_ID     AWS Access Key ID (for Bedrock)
    REACTOR_AI_PROVIDERS_AWS_SECRET_ACCESS_KEY AWS Secret Access Key (for Bedrock)
    REACTOR_AI_PROVIDERS_AWS_BEDROCK_REGION    AWS Bedrock region (default: us-east-1)
    REACTOR_AI_PROVIDERS_AZURE_FOUNDRY_ENDPOINT Azure Foundry endpoint
    REACTOR_AI_PROVIDERS_AZURE_FOUNDRY_API_KEY  Azure Foundry API key
    REACTOR_AI_REGISTRY_OVERLAY        Path to registry overlay TOML
    REACTOR_AI_REGISTRY_URL            URL to fetch registry overlay from
    REACTOR_AI_DEFAULT_ALIAS           Default model alias
    REACTOR_AI_METRICS                 Enable /metrics endpoint (default: false)
    REACTOR_AI_LOG                     Log filter (default: info)
    REACTOR_AI_TIMEOUT_SECS            Request timeout in seconds (default: 120)

For monolith mode:
    REACTOR_AI_AUTH_DATABASE_URL       Auth database URL
    REACTOR_AI_AUTH_DATA_KEY           Column encryption key

For microservices mode:
    REACTOR_AI_AUTH_URL                Auth server URL

See docs/reactor-ai.design.md for full documentation."#,
        reactor_ai::VERSION
    );
}

async fn build_auth_client(config: &AiConfig) -> Result<Arc<dyn AuthClient>> {
    use reactor_ai::config::Deployment;

    match config.deployment {
        Deployment::Monolith => {
            tracing::info!("running in monolith mode");

            // For now, use a stub auth client
            // In a full implementation, this would set up in-process auth
            Err(anyhow::anyhow!(
                "Monolith mode requires auth configuration - use reactor-server for integrated deployment"
            ))
        }
        Deployment::Microservices => {
            tracing::info!("running in microservices mode (RemoteAuthClient)");

            let auth_url = config
                .auth_url
                .as_ref()
                .context("auth_url required for microservices mode")?;

            Ok(Arc::new(reactor_auth::client::RemoteAuthClient::new(
                auth_url.clone(),
            )))
        }
    }
}

fn init_tracing(filter: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}
