//! Reactor Jobs Server.
//!
//! Boots the reactor-jobs HTTP service with:
//! - Configurable auth backend (InProcess or Remote)
//! - In-process scheduler for cron/event/sleep triggers
//! - Worker pool for job execution
//! - Graceful shutdown on SIGTERM/SIGINT
//!
//! Usage:
//!   reactor-jobs-server         Start the server
//!   reactor-jobs-server doctor  Run health checks
//!   reactor-jobs-server --help  Show this help

use anyhow::{Context, Result};
use reactor_cache::PostgresBackend;
use reactor_core::auth::AuthClient;
use reactor_jobs::{router, Deployment, JobsConfig, JobsState};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::watch;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod doctor;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
                println!("reactor-jobs-server {}", VERSION);
                return Ok(());
            }
            other => {
                eprintln!("Unknown command: {}", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    let config = JobsConfig::from_env().context("failed to load config")?;

    init_tracing(&config.log);

    tracing::info!(version = VERSION, "starting reactor-jobs-server");

    // Connect to database
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    // Initialize cache backend
    let cache = Arc::new(PostgresBackend::new(pool.clone()));

    // Run migrations
    cache.migrate().await.context("failed to run cache migrations")?;
    
    let store = reactor_jobs::PgJobsStore::new(pool.clone());
    store.migrate().await.context("failed to run jobs migrations")?;

    // Build auth client
    let auth = build_auth_client(&config).await?;

    // Create state
    let config = Arc::new(config);
    let state = JobsState::new(pool, config.clone(), auth, cache);

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Start scheduler
    let scheduler_state = state.clone();
    let scheduler_shutdown = shutdown_rx.clone();
    let scheduler_interval = Duration::from_millis(config.scheduler_interval_ms);
    let scheduler_handle = tokio::spawn(async move {
        reactor_jobs::scheduler::start_scheduler(scheduler_state, scheduler_shutdown, scheduler_interval).await;
    });

    // Start worker pool
    let worker_state = state.clone();
    let worker_shutdown = shutdown_rx.clone();
    let worker_count = config.worker_count;
    let worker_handle = tokio::spawn(async move {
        let pool = reactor_jobs::worker::WorkerPool::new(worker_count);
        pool.start(worker_state, worker_shutdown, Duration::from_secs(30)).await;
    });

    // Create router
    let app = router(state);

    let listener = TcpListener::bind(config.bind)
        .await
        .context("failed to bind")?;

    tracing::info!(bind = %config.bind, "listening");

    // Start HTTP server with graceful shutdown
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let mut rx = shutdown_rx;
                let _ = rx.changed().await;
            })
            .await
            .expect("server error");
    });

    // Wait for shutdown signal
    shutdown_signal().await;

    // Signal all tasks to shutdown
    let _ = shutdown_tx.send(true);

    // Wait for all tasks to complete
    let _ = tokio::join!(scheduler_handle, worker_handle, server_handle);

    tracing::info!("shutdown complete");
    Ok(())
}

/// Build the appropriate AuthClient based on deployment mode.
async fn build_auth_client(config: &JobsConfig) -> Result<Arc<dyn AuthClient>> {
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
        r#"reactor-jobs-server {}

USAGE:
    reactor-jobs-server [COMMAND]

COMMANDS:
    (none)    Start the HTTP server with scheduler and workers
    doctor    Run health checks on database, auth, functions, and cache

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

ENVIRONMENT:
    REACTOR_JOBS_DATABASE_URL         PostgreSQL connection URL (required)
    REACTOR_JOBS_BIND                 Server bind address (default: 0.0.0.0:8005)
    REACTOR_JOBS_DEPLOYMENT           'monolith' or 'microservices' (default: monolith)
    REACTOR_JOBS_FUNCTIONS_URL        reactor-functions server URL (required)
    REACTOR_JOBS_FUNCTIONS_API_KEY    Internal API key for reactor-functions (required)
    REACTOR_JOBS_DATA_URL             reactor-data server URL (optional)
    REACTOR_JOBS_DATA_API_KEY         Internal API key for reactor-data (optional)
    REACTOR_JOBS_WORKER_COUNT         Number of worker tasks (default: 4)
    REACTOR_JOBS_SCHEDULER_INTERVAL_MS  Scheduler poll interval (default: 1000)
    REACTOR_JOBS_DEFAULT_TIMEOUT_MS   Default job timeout (default: 600000)
    REACTOR_JOBS_MAX_TIMEOUT_MS       Max job timeout (default: 3600000)
    REACTOR_JOBS_WEBHOOK_SECRET       Secret for webhook token encryption (required)
    REACTOR_JOBS_MAX_ORG_CONCURRENT_RUNS  Max concurrent runs per org (default: 50)
    REACTOR_JOBS_METRICS              Enable /metrics endpoint (default: false)
    REACTOR_LOG                       Log filter (default: info)

For monolith mode:
    REACTOR_JOBS_AUTH_DATABASE_URL    Auth database URL
    REACTOR_JOBS_AUTH_DATA_KEY        Auth column encryption key

For microservices mode:
    REACTOR_JOBS_AUTH_URL             Auth server URL
    REACTOR_JOBS_INTERNAL_SECRET      Service-to-service secret

See docs/reactor-jobs.design.md for full documentation."#,
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
