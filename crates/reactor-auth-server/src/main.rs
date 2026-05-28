//! Standalone authentication server for Reactor.cloud

use anyhow::{Context, Result};
use reactor_auth::{migrator, router, AuthConfig, AuthState};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = AuthConfig::load().context("failed to load configuration")?;

    // Initialize tracing
    init_tracing(&config.log);

    tracing::info!(version = VERSION, "starting reactor-auth-server");

    // Validate configuration
    config.validate().context("invalid configuration")?;

    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    tracing::info!("connected to database");

    // Run migrations
    tracing::info!("running migrations");
    migrator()
        .run(&pool)
        .await
        .context("failed to run migrations")?;
    tracing::info!("migrations complete");

    // Create application state
    let state = AuthState::from_pool(pool, config.clone()).context("failed to create auth state")?;

    // Ensure we have an active signing key (auto-generates one if needed)
    state
        .keyring
        .ensure_active_key()
        .await
        .context("failed to ensure signing key")?;
    tracing::info!("signing key ready");

    // Build router
    let app = router(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .context("failed to bind to address")?;

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

fn init_tracing(filter: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}
