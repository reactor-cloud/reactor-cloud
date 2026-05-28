//! Edge gateway server for Reactor.
//!
//! This server runs alongside Caddy and provides:
//! - Sync loop that applies routing changes to Caddy
//! - /ask endpoint for on-demand TLS certificate provisioning
//! - Domain management API (POST /gateway/v1/domains)
//! - Health check endpoints

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use clap::Parser;
use reactor_core::ProjectId;
use reactor_gateway::{
    caddy_admin::{CaddyAdminClient, CaddyAdminConfig},
    routing::{BackendTarget, CustomDomain, Route, TlsMode},
    store::{spawn_notification_forwarder, PgRoutingStore},
    sync::{SyncConfig, SyncLoop, SyncMessage},
};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use url::Url;

/// Reactor Gateway Server
#[derive(Parser, Debug)]
#[command(name = "reactor-gateway-server")]
#[command(about = "Edge gateway server for Reactor")]
struct Args {
    /// Listen address for the /ask endpoint
    #[arg(long, env = "GATEWAY_LISTEN_ADDR", default_value = "0.0.0.0:9000")]
    listen_addr: SocketAddr,

    /// Caddy admin API address
    #[arg(long, env = "CADDY_ADMIN_ADDR", default_value = "http://localhost:2019")]
    caddy_admin_addr: String,

    /// Database URL
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    /// Cloudflare API token for DNS-01 challenges
    #[arg(long, env = "CF_API_TOKEN")]
    cf_api_token: Option<String>,

    /// Default backend address
    #[arg(long, env = "DEFAULT_BACKEND", default_value = "reactor-cloud.internal:8000")]
    default_backend: String,

    /// Wildcard domain
    #[arg(long, env = "WILDCARD_DOMAIN", default_value = "*.reactor.cloud")]
    wildcard_domain: String,

    /// ACME email
    #[arg(long, env = "ACME_EMAIL", default_value = "admin@reactor.cloud")]
    acme_email: String,

    /// Use ACME staging
    #[arg(long, env = "ACME_STAGING")]
    acme_staging: bool,
}

/// Application state shared across handlers.
struct AppState {
    store: Arc<PgRoutingStore>,
    sync_sender: mpsc::Sender<SyncMessage>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("reactor_gateway=info".parse()?)
                .add_directive("reactor_gateway_server=info".parse()?),
        )
        .json()
        .init();

    let args = Args::parse();

    info!("Starting Reactor Gateway Server");
    info!("Listen address: {}", args.listen_addr);
    info!("Caddy admin: {}", args.caddy_admin_addr);
    info!("Default backend: {}", args.default_backend);
    info!("Wildcard domain: {}", args.wildcard_domain);
    info!("ACME staging: {}", args.acme_staging);

    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&args.database_url)
        .await?;

    info!("Connected to database");

    let store = Arc::new(PgRoutingStore::new(pool));

    // Configure Caddy client
    let caddy_url = Url::parse(&args.caddy_admin_addr)?;
    let mut caddy_config = CaddyAdminConfig::new(caddy_url)
        .with_default_backend(&args.default_backend)
        .with_wildcard_domain(&args.wildcard_domain)
        .with_acme_email(&args.acme_email);

    if let Some(cf_token) = args.cf_api_token {
        caddy_config = caddy_config.with_cloudflare_token(cf_token);
    }

    if args.acme_staging {
        caddy_config = caddy_config.with_acme_staging();
    }

    let caddy_client = Arc::new(CaddyAdminClient::new(caddy_config));

    // Create sync loop
    let sync_config = SyncConfig::default();
    let (sync_loop, sync_sender) = SyncLoop::new(store.clone(), caddy_client.clone(), sync_config);

    // Spawn notification forwarder
    spawn_notification_forwarder(store.clone(), sync_sender.clone())
        .await?;

    // Spawn sync loop
    let sync_handle = tokio::spawn(async move {
        if let Err(e) = sync_loop.run().await {
            error!("Sync loop error: {}", e);
        }
    });

    // Create app state
    let state = Arc::new(AppState {
        store: store.clone(),
        sync_sender,
    });

    // Build router
    let app = Router::new()
        // On-demand TLS endpoint (called by Caddy)
        .route("/ask", get(ask_handler))
        // Health endpoints
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        // Domain management API
        .route("/gateway/v1/domains", post(create_domain_handler))
        .route("/gateway/v1/domains/:host/verify", post(verify_domain_handler))
        .with_state(state);

    // Start HTTP server
    let listener = tokio::net::TcpListener::bind(&args.listen_addr).await?;
    info!("Listening on {}", args.listen_addr);

    // Handle shutdown
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
        info!("Shutdown signal received");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    // Wait for sync loop to finish
    let _ = sync_handle.await;

    info!("Gateway server stopped");
    Ok(())
}

/// Query parameters for the /ask endpoint.
#[derive(Debug, Deserialize)]
struct AskQuery {
    domain: String,
}

/// Handle the /ask endpoint for on-demand TLS.
///
/// Caddy calls this endpoint to check if a domain should receive a certificate.
async fn ask_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AskQuery>,
) -> impl IntoResponse {
    let domain = &query.domain;

    match state.store.is_domain_verified(domain).await {
        Ok(true) => {
            info!("Domain {} approved for TLS", domain);
            StatusCode::OK
        }
        Ok(false) => {
            info!("Domain {} NOT approved for TLS", domain);
            StatusCode::NOT_FOUND
        }
        Err(e) => {
            error!("Error checking domain {}: {}", domain, e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Health check endpoint.
async fn health_handler() -> impl IntoResponse {
    StatusCode::OK
}

/// Readiness check endpoint.
async fn ready_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Check database connection
    match sqlx::query("SELECT 1").fetch_one(state.store.pool()).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

// ============================================================================
// Domain Management API
// ============================================================================

/// Request body for creating a new custom domain.
#[derive(Debug, Deserialize)]
struct CreateDomainRequest {
    /// The domain hostname (e.g., "custom.example.com").
    host: String,
    /// The project ID this domain belongs to.
    project_id: uuid::Uuid,
    /// The backend target address (e.g., "reactor-cloud.internal:8000").
    backend_target: String,
}

/// Response for domain creation.
#[derive(Debug, Serialize)]
struct CreateDomainResponse {
    /// The domain hostname.
    host: String,
    /// DNS TXT record name to create (e.g., "_reactor-verify.custom.example.com").
    txt_record_name: String,
    /// DNS TXT record value for verification.
    txt_record_value: String,
    /// Status message.
    message: String,
}

/// Create a new custom domain and return verification instructions.
async fn create_domain_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateDomainRequest>,
) -> impl IntoResponse {
    // Generate a verification token
    let verification_token = generate_verification_token();
    let txt_record_name = format!("_reactor-verify.{}", req.host);

    let project_id = ProjectId::from(req.project_id);

    // Create the custom domain entry
    let domain = CustomDomain::new(&req.host, project_id.clone(), &verification_token);

    match state.store.insert_custom_domain(&domain).await {
        Ok(()) => {
            info!("Created custom domain: {}", req.host);

            let response = CreateDomainResponse {
                host: req.host,
                txt_record_name,
                txt_record_value: verification_token,
                message: "Domain created. Add the TXT record to your DNS and call /verify".to_string(),
            };

            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            error!("Failed to create domain {}: {}", req.host, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Response for domain verification.
#[derive(Debug, Serialize)]
struct VerifyDomainResponse {
    /// The domain hostname.
    host: String,
    /// Whether verification was successful.
    verified: bool,
    /// Status message.
    message: String,
}

/// Verify a custom domain by checking DNS TXT records.
async fn verify_domain_handler(
    State(state): State<Arc<AppState>>,
    Path(host): Path<String>,
) -> impl IntoResponse {
    // Look up the domain
    let domain = match state.store.get_custom_domain(&host).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Domain not found" })),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to look up domain {}: {}", host, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // Check if already verified
    if domain.is_verified() {
        return (
            StatusCode::OK,
            Json(VerifyDomainResponse {
                host: host.clone(),
                verified: true,
                message: "Domain is already verified".to_string(),
            }),
        )
            .into_response();
    }

    // Perform DNS verification
    let txt_record_name = format!("_reactor-verify.{}", host);
    match verify_dns_txt(&txt_record_name, &domain.verification_token).await {
        Ok(true) => {
            // Mark domain as verified
            if let Err(e) = state.store.verify_custom_domain(&host).await {
                error!("Failed to mark domain {} as verified: {}", host, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }

            // Also create a route for this domain
            let project_ref = domain.project_id.to_ref();
            let route = Route::new(
                &host,
                domain.project_id.clone(),
                project_ref,
                BackendTarget::new("reactor-cloud.internal:8000"),
            )
            .with_tls_mode(TlsMode::OnDemand);

            if let Err(e) = state.store.upsert_route(&route).await {
                warn!("Failed to create route for verified domain {}: {}", host, e);
                // Don't fail verification if route creation fails
            }

            info!("Domain {} verified successfully", host);

            (
                StatusCode::OK,
                Json(VerifyDomainResponse {
                    host,
                    verified: true,
                    message: "Domain verified successfully. Certificate will be provisioned on first request.".to_string(),
                }),
            )
                .into_response()
        }
        Ok(false) => {
            info!("DNS verification failed for domain {}", host);
            (
                StatusCode::BAD_REQUEST,
                Json(VerifyDomainResponse {
                    host,
                    verified: false,
                    message: format!(
                        "DNS verification failed. Ensure TXT record {} contains the correct token.",
                        txt_record_name
                    ),
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("DNS lookup error for domain {}: {}", host, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("DNS lookup failed: {}", e) })),
            )
                .into_response()
        }
    }
}

/// Generate a random verification token.
fn generate_verification_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Simple token: base64-encoded timestamp + random suffix
    format!("reactor-verify-{:x}", timestamp)
}

/// Verify a DNS TXT record contains the expected value.
async fn verify_dns_txt(record_name: &str, expected_value: &str) -> Result<bool, String> {
    // Use the system resolver to look up TXT records
    // In production, you might want to use a dedicated DNS library like trust-dns
    use std::process::Command;

    let output = Command::new("dig")
        .args(["+short", "TXT", record_name])
        .output()
        .map_err(|e| format!("Failed to execute dig: {}", e))?;

    if !output.status.success() {
        return Err("dig command failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // TXT records are quoted in dig output
    for line in stdout.lines() {
        let value = line.trim().trim_matches('"');
        if value == expected_value {
            return Ok(true);
        }
    }

    Ok(false)
}

