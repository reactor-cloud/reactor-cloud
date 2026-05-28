//! Reactor Analytics Server
//!
//! Standalone server binary for the analytics capability.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use figment::{providers::Env, Figment};
use reactor_analytics::{
    config::AnalyticsConfig,
    ingest::{create_batcher_channel, Batcher, BatcherConfig},
    router, AnalyticsState, PgAnalyticsStore,
};
use reactor_auth::client::RemoteAuthClient;
use reactor_core::auth::AuthClient;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod cli;

#[derive(Parser)]
#[command(name = "reactor-analytics-server")]
#[command(about = "Reactor Analytics Server")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the analytics server
    Serve,
    /// Run database migrations
    Migrate,
    /// Check server health and configuration
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    init_tracing();

    match args.command {
        Commands::Serve => serve().await,
        Commands::Migrate => migrate().await,
        Commands::Doctor => cli::doctor::run().await,
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,reactor_analytics=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn serve() -> Result<()> {
    let config = load_config()?;

    tracing::info!(bind = %config.bind, "starting analytics server");

    let pool = PgPoolOptions::new()
        .max_connections(50)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    let store = Arc::new(PgAnalyticsStore::new(pool));
    let auth = create_auth_client(&config).await?;
    let config = Arc::new(config);

    // Create batcher channel
    let (batcher_tx, batcher_rx) = create_batcher_channel(&config);

    // Spawn background batcher task
    let batcher_store = store.clone();
    let batcher_config = BatcherConfig::from(config.as_ref());
    tokio::spawn(async move {
        let batcher = Batcher::new(batcher_store, batcher_config, batcher_rx);
        batcher.run().await;
    });

    let state = AnalyticsState::new(store, config.clone(), auth, batcher_tx);
    let app = router(state);

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .context("failed to bind to address")?;

    tracing::info!("analytics server listening on {}", config.bind);

    axum::serve(listener, app)
        .await
        .context("server error")?;

    Ok(())
}

async fn migrate() -> Result<()> {
    let config = load_config()?;

    tracing::info!("running analytics migrations");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    let store = PgAnalyticsStore::new(pool);
    store.migrate().await.context("migration failed")?;

    tracing::info!("migrations completed successfully");

    Ok(())
}

fn load_config() -> Result<AnalyticsConfig> {
    Figment::new()
        .merge(Env::prefixed("REACTOR_ANALYTICS_").split("_"))
        .merge(Env::prefixed("REACTOR_").split("_"))
        .extract()
        .context("failed to load configuration")
}

async fn create_auth_client(config: &AnalyticsConfig) -> Result<Arc<dyn AuthClient>> {
    match &config.auth_url {
        Some(url) => {
            tracing::info!(auth_url = %url, "using remote auth client");
            let client = RemoteAuthClient::new(url.clone());
            Ok(Arc::new(client))
        }
        None => {
            anyhow::bail!("auth_url must be configured for analytics server")
        }
    }
}
