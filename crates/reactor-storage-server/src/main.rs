//! Reactor Storage Server.
//!
//! Boots the reactor-storage HTTP service with:
//! - Configurable auth backend (InProcess or Remote)
//! - Graceful shutdown on SIGTERM/SIGINT
//!
//! Usage:
//!   reactor-storage-server         Start the server
//!   reactor-storage-server doctor  Run health checks
//!   reactor-storage-server --help  Show this help
//!
//! See `docs/reactor-storage.design.md` for configuration options.

mod doctor;

use anyhow::{Context, Result};
use reactor_core::auth::AuthClient;
use reactor_storage::{router, Deployment, StorageConfig, StorageState};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Check for subcommand
    if args.len() > 1 {
        match args[1].as_str() {
            "doctor" => return doctor::run().await,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--version" | "-V" => {
                println!("reactor-storage-server {}", reactor_storage::VERSION);
                return Ok(());
            }
            other => {
                eprintln!("Unknown command: {}", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    let config = StorageConfig::from_env().context("failed to load config")?;

    init_tracing(&config.log);

    tracing::info!(
        version = reactor_storage::VERSION,
        "starting reactor-storage-server"
    );

    // Connect to database
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    // TODO: PR 4 - Run metadata migrations here
    // store.run_metadata_migrations().await?;

    let auth = build_auth_client(&config).await?;
    let config = Arc::new(config);
    let state = StorageState::new(pool, config.clone(), auth);
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

/// Wait for shutdown signal (SIGTERM or SIGINT).
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
        r#"reactor-storage-server {}

USAGE:
    reactor-storage-server [COMMAND]

COMMANDS:
    (none)    Start the HTTP server
    doctor    Run health checks on database and auth connectivity

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

ENVIRONMENT:
    REACTOR_STORAGE_DATABASE_URL       PostgreSQL connection URL (required)
    REACTOR_STORAGE_BIND               Server bind address (default: 127.0.0.1:8082)
    REACTOR_STORAGE_DEPLOYMENT         'monolith' or 'microservices' (default: monolith)
    REACTOR_STORAGE_FS_BASE_PATH       Base path for filesystem storage
    REACTOR_STORAGE_S3_BUCKET          S3 bucket name (when using S3 backend)
    REACTOR_STORAGE_S3_REGION          S3 region
    REACTOR_STORAGE_S3_ENDPOINT        S3 endpoint override (for MinIO)
    REACTOR_STORAGE_SIGNING_SECRET     Secret for HMAC-signed URLs
    REACTOR_STORAGE_SIGNED_URL_EXPIRY_SECS  URL expiry (default: 3600)
    REACTOR_STORAGE_MAX_UPLOAD_SIZE    Max upload bytes (default: 104857600)
    REACTOR_STORAGE_METRICS            Enable /metrics endpoint (default: false)
    REACTOR_STORAGE_LOG                Log filter (default: info)

For monolith mode:
    REACTOR_STORAGE_AUTH_DATABASE_URL  Auth database URL
    REACTOR_STORAGE_AUTH_DATA_KEY      Column encryption key

For microservices mode:
    REACTOR_STORAGE_AUTH_URL           Auth server URL

See docs/reactor-storage.design.md for full documentation."#,
        reactor_storage::VERSION
    );
}

/// Build the appropriate AuthClient based on deployment mode.
async fn build_auth_client(config: &StorageConfig) -> Result<Arc<dyn AuthClient>> {
    match config.deployment {
        Deployment::Monolith => {
            tracing::info!("running in monolith mode (InProcessAuthClient)");

            let auth_db_url = config
                .auth_database_url
                .as_ref()
                .context("auth_database_url required for monolith mode")?;

            let data_key = config
                .auth_data_key
                .as_ref()
                .context("auth_data_key required for monolith mode")?;

            let pool = sqlx::PgPool::connect(auth_db_url)
                .await
                .context("failed to connect to auth database")?;

            let store = Arc::new(reactor_auth::store::PgIdentityStore::new(pool));
            let encryptor = reactor_auth::crypto::ColumnEncryptor::new(data_key)
                .context("failed to create encryptor")?;
            let keyring = Arc::new(reactor_auth::token::KeyringManager::new(
                store.clone(),
                encryptor,
            ));
            let email_sender = Arc::new(reactor_auth::email::NoopSender);

            // Load auth config from environment (same envs as reactor-auth-server)
            let auth_config =
                reactor_auth::AuthConfig::load().context("failed to load auth config")?;
            let auth_config = Arc::new(auth_config);

            let service = Arc::new(reactor_auth::AuthService::new(
                store,
                keyring,
                email_sender,
                auth_config,
            ));

            Ok(Arc::new(reactor_auth::client::InProcessAuthClient::new(
                service,
            )))
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
