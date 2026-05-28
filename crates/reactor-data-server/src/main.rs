//! Reactor Data server binary.
//!
//! Boots the reactor-data HTTP service with:
//! - Configurable auth backend (InProcess or Remote)
//! - Metadata migrations applied on startup
//! - User migrations from project directory
//! - Graceful shutdown on SIGTERM/SIGINT
//!
//! Usage:
//!   reactor-data-server         Start the server
//!   reactor-data-server doctor  Run health checks
//!   reactor-data-server --help  Show this help
//!
//! See `docs/reactor-data.design.md` for configuration options.

mod doctor;

use anyhow::{Context, Result};
use reactor_core::auth::AuthClient;
use reactor_data::{
    router, DataConfig, DataState, DataStore, Deployment, FilesystemSource, MigrationRunner,
    PgDataStore,
};
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
                println!("reactor-data-server {}", reactor_data::VERSION);
                return Ok(());
            }
            other => {
                eprintln!("Unknown command: {}", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    let config = DataConfig::from_env().context("failed to load config")?;

    init_tracing(&config.log);

    tracing::info!(
        version = reactor_data::VERSION,
        "starting reactor-data-server"
    );

    // Connect to data database
    let data_pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .context("failed to connect to data database")?;

    let store = Arc::new(PgDataStore::new(data_pool.clone()));

    // Run metadata migrations
    tracing::info!("running metadata migrations");
    store
        .run_metadata_migrations()
        .await
        .context("failed to run metadata migrations")?;

    // Run user migrations if migrations_dir is set and run_migrations is true
    if config.run_migrations {
        if let Some(ref migrations_dir) = config.migrations_dir {
            tracing::info!(dir = %migrations_dir.display(), "running user migrations");
            let source = FilesystemSource::new(migrations_dir);
            let runner = MigrationRunner::new(source, &config.user_schema);
            let applied = runner
                .run(&data_pool, store.as_ref())
                .await
                .context("failed to run user migrations")?;
            tracing::info!(count = applied, "user migrations complete");
        }
    }

    let auth = build_auth_client(&config).await?;
    let config = Arc::new(config);
    let state = DataState::new(store, auth, config.clone());
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
        r#"reactor-data-server {}

USAGE:
    reactor-data-server [COMMAND]

COMMANDS:
    (none)    Start the HTTP server
    doctor    Run health checks on database and auth connectivity

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

ENVIRONMENT:
    REACTOR_DATA_DATABASE_URL    PostgreSQL connection URL (required)
    REACTOR_DATA_BIND            Server bind address (default: 127.0.0.1:8081)
    REACTOR_DATA_DEPLOYMENT      'monolith' or 'microservices' (default: monolith)
    REACTOR_DATA_MIGRATIONS_DIR  Path to user migrations directory
    REACTOR_DATA_USER_SCHEMA     User schema name (default: public)
    REACTOR_DATA_RUN_MIGRATIONS  Apply migrations on startup (default: true)
    REACTOR_DATA_METRICS         Enable /metrics endpoint (default: false)
    REACTOR_DATA_LOG             Log filter (default: info)

For monolith mode:
    REACTOR_DATA_AUTH_DATABASE_URL    Auth database URL
    REACTOR_DATA_AUTH_DATA_KEY        Column encryption key

For microservices mode:
    REACTOR_DATA_AUTH_URL             Auth server URL

See docs/reactor-data.design.md for full documentation."#,
        reactor_data::VERSION
    );
}

/// Build the appropriate AuthClient based on deployment mode.
async fn build_auth_client(config: &DataConfig) -> Result<Arc<dyn AuthClient>> {
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
