//! Reactor Functions Server.
//!
//! Boots the reactor-functions HTTP service with:
//! - Configurable auth backend (InProcess or Remote)
//! - Multiple runtime adapters (wasm, bun, lambda)
//! - Graceful shutdown on SIGTERM/SIGINT
//!
//! Usage:
//!   reactor-functions-server         Start the server
//!   reactor-functions-server doctor  Run health checks
//!   reactor-functions-server --help  Show this help
//!
//! See `docs/reactor-functions.design.md` for configuration options.

use anyhow::{Context, Result};
use reactor_core::auth::AuthClient;
use reactor_functions::{router, Deployment, FunctionsConfig, FunctionsState, RuntimeRegistry};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg(feature = "runtime-wasm")]
use reactor_functions::{WasmRuntime, WasmRuntimeConfig};

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
                println!("reactor-functions-server {}", VERSION);
                return Ok(());
            }
            other => {
                eprintln!("Unknown command: {}", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    let config = FunctionsConfig::from_env().context("failed to load config")?;

    init_tracing(&config.log);

    tracing::info!(
        version = VERSION,
        "starting reactor-functions-server"
    );

    // Connect to database
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    // TODO: PR 3 - Run metadata migrations here
    // store.run_metadata_migrations().await?;

    // TODO: PR 5 - Initialize storage client and ensure system bucket exists

    // Initialize runtime registry and register available runtimes
    let runtimes = Arc::new(RuntimeRegistry::new());
    register_runtimes(&runtimes, &config).await;

    let auth = build_auth_client(&config).await?;
    let config = Arc::new(config);
    let state = FunctionsState::new(pool, config.clone(), auth, runtimes);
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

/// Build the appropriate AuthClient based on deployment mode.
async fn build_auth_client(config: &FunctionsConfig) -> Result<Arc<dyn AuthClient>> {
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

/// Register available runtimes based on enabled features.
#[allow(unused_variables)]
async fn register_runtimes(registry: &RuntimeRegistry, config: &FunctionsConfig) {
    #[cfg(feature = "runtime-wasm")]
    {
        use std::path::PathBuf;
        let workdir = PathBuf::from(&config.workdir);
        let wasm_config = WasmRuntimeConfig {
            cache_dir: workdir.join("wasm-cache"),
        };
        let wasm_runtime = Arc::new(WasmRuntime::new(wasm_config));
        registry.register(wasm_runtime).await;
        tracing::info!("registered wasm runtime");
    }

    #[cfg(feature = "runtime-bun")]
    {
        use reactor_functions::{BunRuntime, BunRuntimeConfig};
        use std::path::PathBuf;
        let workdir = PathBuf::from(&config.workdir);
        let bun_config = BunRuntimeConfig {
            bun_bin: config.bun_bin.clone(),
            workdir: workdir.join("bun"),
            idle_ttl_secs: config.bun_idle_ttl_secs,
            max_instances_per_fn: config.bun_max_instances_per_fn,
        };
        let bun_runtime = Arc::new(BunRuntime::new(bun_config));
        registry.register(bun_runtime).await;
        tracing::info!("registered bun runtime");
    }

    #[cfg(feature = "runtime-lambda")]
    {
        use reactor_functions::{LambdaRuntime, LambdaRuntimeConfig};
        if let (Some(region), Some(role_arn), Some(bundle_bucket)) = (
            &config.lambda_region,
            &config.lambda_role_arn,
            &config.lambda_bundle_s3_bucket,
        ) {
            let lambda_config = LambdaRuntimeConfig {
                region: region.clone(),
                role_arn: role_arn.clone(),
                bundle_s3_bucket: bundle_bucket.clone(),
                lwa_layer_arn: config.lambda_lwa_layer_arn.clone().unwrap_or_default(),
                log_group_prefix: config.lambda_log_group_prefix.clone(),
            };
            let lambda_runtime = Arc::new(LambdaRuntime::new(lambda_config));
            registry.register(lambda_runtime).await;
            tracing::info!("registered lambda runtime");
        } else {
            tracing::info!("lambda runtime enabled but not configured (missing region/role/bucket)");
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
        r#"reactor-functions-server {}

USAGE:
    reactor-functions-server [COMMAND]

COMMANDS:
    (none)    Start the HTTP server
    doctor    Run health checks on database, auth, storage, and runtimes

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

ENVIRONMENT:
    REACTOR_FUNCTIONS_DATABASE_URL       PostgreSQL connection URL (required)
    REACTOR_FUNCTIONS_BIND               Server bind address (default: 127.0.0.1:8083)
    REACTOR_FUNCTIONS_DEPLOYMENT         'monolith' or 'microservices' (default: monolith)
    REACTOR_FUNCTIONS_WORKDIR            Working directory (default: /var/lib/reactor-functions)
    REACTOR_FUNCTIONS_STORAGE_URL        Storage service URL (required)
    REACTOR_FUNCTIONS_STORAGE_API_KEY    Storage API key (required)
    REACTOR_FUNCTIONS_DATA_KEY           Column encryption key (required)
    REACTOR_FUNCTIONS_INVOKE_DEFAULT_TIMEOUT_MS   Default timeout (default: 30000)
    REACTOR_FUNCTIONS_INVOKE_MAX_TIMEOUT_MS       Max timeout (default: 300000)
    REACTOR_FUNCTIONS_BUNDLE_MAX_BYTES   Max bundle size (default: 52428800)
    REACTOR_FUNCTIONS_METRICS            Enable /metrics endpoint (default: false)
    REACTOR_FUNCTIONS_LOG                Log filter (default: info)

Bun runtime:
    REACTOR_FUNCTIONS_BUN_BIN            Path to bun binary (default: bun)
    REACTOR_FUNCTIONS_BUN_IDLE_TTL_SECS  Warm pool TTL (default: 300)
    REACTOR_FUNCTIONS_BUN_MAX_INSTANCES_PER_FN   Max warm instances (default: 8)

Lambda runtime:
    REACTOR_FUNCTIONS_LAMBDA_REGION              AWS region
    REACTOR_FUNCTIONS_LAMBDA_ROLE_ARN            Execution role ARN
    REACTOR_FUNCTIONS_LAMBDA_BUNDLE_S3_BUCKET    S3 bucket for bundles
    REACTOR_FUNCTIONS_LAMBDA_LWA_LAYER_ARN       Lambda Web Adapter layer ARN
    REACTOR_FUNCTIONS_LAMBDA_LOG_GROUP_PREFIX    CloudWatch log prefix

For monolith mode:
    REACTOR_FUNCTIONS_AUTH_DATABASE_URL  Auth database URL
    REACTOR_FUNCTIONS_AUTH_DATA_KEY      Auth column encryption key

For microservices mode:
    REACTOR_FUNCTIONS_AUTH_URL           Auth server URL
    REACTOR_FUNCTIONS_INTERNAL_SECRET    Service-to-service secret

See docs/reactor-functions.design.md for full documentation."#,
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
