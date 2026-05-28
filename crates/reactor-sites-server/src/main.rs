//! Reactor Sites Server.
//!
//! Boots the reactor-sites HTTP service with:
//! - Configurable auth backend (InProcess or Remote)
//! - Framework adapters (static, hono, nextjs)
//! - Graceful shutdown on SIGTERM/SIGINT
//!
//! Usage:
//!   reactor-sites-server         Start the server
//!   reactor-sites-server doctor  Run health checks
//!   reactor-sites-server --help  Show this help

use anyhow::{Context, Result};
use reactor_cache::PostgresBackend;
use reactor_core::auth::AuthClient;
use reactor_sites::{
    dispatch::FunctionsClient,
    router, Deployment, SitesConfig, SitesState,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod doctor;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
                println!("reactor-sites-server {}", VERSION);
                return Ok(());
            }
            other => {
                eprintln!("Unknown command: {}", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    let config = SitesConfig::from_env().context("failed to load config")?;

    init_tracing(&config.log);

    tracing::info!(version = VERSION, "starting reactor-sites-server");

    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    let functions = FunctionsClient::new(config.functions_url.clone(), config.functions_api_key.clone());

    let storage = reactor_sites::dispatch::static_dispatch::StorageClient::new(
        config.storage_url.clone(),
        config.storage_api_key.clone(),
    );

    storage
        .ensure_system_bucket()
        .await
        .context("failed to ensure system bucket")?;

    let cache = Arc::new(PostgresBackend::new(pool.clone()));

    let auth = build_auth_client(&config).await?;

    let config = Arc::new(config);
    let state = SitesState::new(pool, config.clone(), auth, functions, storage, cache);
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

async fn build_auth_client(config: &SitesConfig) -> Result<Arc<dyn AuthClient>> {
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
        r#"reactor-sites-server {}

USAGE:
    reactor-sites-server [COMMAND]

COMMANDS:
    (none)    Start the HTTP server
    doctor    Run health checks on database, auth, functions, and storage

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

ENVIRONMENT:
    REACTOR_SITES_DATABASE_URL       PostgreSQL connection URL (required)
    REACTOR_SITES_BIND               Server bind address (default: 127.0.0.1:8006)
    REACTOR_SITES_DEPLOYMENT         'monolith' or 'microservices' (default: monolith)
    REACTOR_SITES_WORKDIR            Working directory (default: /var/lib/reactor-sites)
    REACTOR_SITES_FUNCTIONS_URL      Functions service URL (required)
    REACTOR_SITES_FUNCTIONS_API_KEY  Functions API key (required)
    REACTOR_SITES_STORAGE_URL        Storage service URL (required)
    REACTOR_SITES_STORAGE_API_KEY    Storage API key (required)
    REACTOR_SITES_REVALIDATION_SECRET  ISR revalidation secret (required)
    REACTOR_SITES_STATIC_MAX_FILES   Max static files per deployment (default: 50000)
    REACTOR_SITES_STATIC_MAX_BYTES   Max static bytes per deployment (default: 536870912)
    REACTOR_SITES_ISR_DEFAULT_TTL_SECS  Default ISR TTL (default: 3600)
    REACTOR_SITES_PREVIEW_SUBDOMAIN  Preview subdomain prefix (default: preview)
    REACTOR_SITES_METRICS            Enable /metrics endpoint (default: false)
    REACTOR_SITES_INVOCATION_SAMPLE_RATE  Invocation sample rate (default: 0.01)
    REACTOR_SITES_LOG                Log filter (default: info)

For monolith mode:
    REACTOR_SITES_AUTH_DATABASE_URL  Auth database URL
    REACTOR_SITES_AUTH_DATA_KEY      Auth column encryption key

For microservices mode:
    REACTOR_SITES_AUTH_URL           Auth server URL
    REACTOR_SITES_INTERNAL_SECRET    Service-to-service secret

See docs/reactor-sites.design.md for full documentation."#,
        VERSION
    );
}

fn init_tracing(filter: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}
