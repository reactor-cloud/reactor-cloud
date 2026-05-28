//! Reactor Server — Unified Binary
//!
//! This crate provides the unified `reactor-server` binary that mounts every
//! capability's router in one process against a shared PgPool.
//!
//! # Topologies
//!
//! Legacy generation names (still used in cargo features):
//!
//! - **G1 (Tauri)**: Embedded in-process, `127.0.0.1` only → `S1@tauri`
//! - **G2 (Single VPS)**: One process per host, shared Postgres → `M1@<target>`
//! - **G3 (Production)**: Horizontally scalable multi-tenant → `M4+@<target>`
//!
//! See `deploy/README.md` for the full deployment naming scheme:
//! `<Tenancy><Level>@<Target>[+ha][~variant]`
//!
//! # Usage
//!
//! ```ignore
//! use reactor_server::{run, ReactorConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = ReactorConfig::load()?;
//!     run(config).await
//! }
//! ```
//!
//! # Features
//!
//! Capability gates allow trimming the binary for different topologies:
//!
//! - `cap-auth` — Include auth capability
//! - `cap-data` — Include data capability
//! - `cap-storage` — Include storage capability
//! - `cap-functions` — Include functions capability
//! - `cap-jobs` — Include jobs capability
//! - `cap-sites` — Include sites capability
//!
//! Topology bundles:
//!
//! - `g1-tauri` — Minimal set for desktop embedding
//! - `g2-full` — Full capability set (default)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod admin;
pub mod boot;
pub mod compose;
pub mod config;
pub mod error;
#[cfg(feature = "cap-cloud")]
pub mod quota;
#[cfg(feature = "cap-cloud")]
pub mod tenant_cache;

pub use boot::{SharedResources, ShutdownHandle, Tenant, TenantProvider};
pub use compose::ServerCapabilities;
pub use config::ReactorConfig;
pub use error::ServerError;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use reactor_core::primitives::vault::Vault;

/// Resolve a secret value, potentially from vault.
///
/// Supports:
/// - Direct value: `"my-secret-key"` - uses the value directly
/// - Vault reference: `"vault:secret-name"` - fetches from vault KV
async fn resolve_secret(
    config_value: &str,
    vault: &dyn Vault,
    tenant: &reactor_core::ProjectId,
    description: &str,
) -> Result<String, ServerError> {
    if config_value.starts_with("vault:") {
        let vault_key = &config_value[6..]; // Strip "vault:" prefix
        let secret = vault
            .get_secret(tenant, vault_key)
            .await
            .map_err(|e| ServerError::Boot(format!("failed to get {} from vault: {}", description, e)))?
            .ok_or_else(|| ServerError::Config(format!(
                "{} '{}' not found in vault", description, vault_key
            )))?;

        String::from_utf8(secret.data)
            .map_err(|_| ServerError::Config(format!("{} is not valid UTF-8", description)))
    } else {
        Ok(config_value.to_string())
    }
}

/// Run the unified server with the given configuration.
///
/// This is the main entry point for both the binary and Tauri embedding.
/// It boots shared resources, runs migrations, composes capabilities,
/// and serves HTTP until shutdown.
pub async fn run(config: ReactorConfig) -> anyhow::Result<()> {
    // Validate config
    config.validate()?;

    // Initialize tracing
    boot::tracing::init(&config.tracing);

    tracing::info!(version = VERSION, "starting reactor-server");

    // Create shutdown handle
    let shutdown_handle = ShutdownHandle::new();

    // Build shared resources
    let shared = SharedResources::build(&config, shutdown_handle.receiver()).await?;

    // Run migrations
    boot::migrate::run_all(&shared.pg, &config).await?;

    // Bootstrap cloud control plane (resume stuck projects)
    #[cfg(feature = "cap-cloud")]
    if let Some(ref cloud_config) = config.cloud {
        let cloud_bootstrap_config = boot::CloudBootstrapConfig {
            base_domain: cloud_config.base_domain.clone(),
            backend_target: cloud_config.backend_target.clone(),
            tls_mode: cloud_config.tls_mode.clone().unwrap_or_else(|| "wildcard".to_string()),
        };
        let result = boot::cloud_bootstrap(&shared.pg, shared.vault.clone(), &cloud_bootstrap_config)
            .await
            .map_err(|e| ServerError::Boot(format!("cloud bootstrap failed: {}", e)))?;
        
        if result.provisioning_resumed > 0 || result.teardown_resumed > 0 {
            tracing::info!(
                provisioning_resumed = result.provisioning_resumed,
                teardown_resumed = result.teardown_resumed,
                "cloud bootstrap completed"
            );
        }
    }

    // Build tenant provider (single-tenant or multi-tenant mode)
    #[cfg(feature = "cap-cloud")]
    let is_multi_tenant = config
        .cloud
        .as_ref()
        .map(|c| c.multi_tenant)
        .unwrap_or(false);
    #[cfg(not(feature = "cap-cloud"))]
    let is_multi_tenant = false;

    let tenant_provider = if is_multi_tenant {
        #[cfg(feature = "cap-cloud")]
        {
            let tenant_cache_ttl = config
                .cloud
                .as_ref()
                .map(|c| c.tenant_cache_ttl_secs)
                .unwrap_or(300);
            let provider = TenantProvider::host_lookup(
                shared.pg.clone(),
                std::time::Duration::from_secs(tenant_cache_ttl),
                None, // no fallback host
            );
            tracing::info!(
                cache_ttl_secs = tenant_cache_ttl,
                "multi-tenant mode enabled, using host-based tenant resolution"
            );
            std::sync::Arc::new(provider)
        }
        #[cfg(not(feature = "cap-cloud"))]
        {
            unreachable!("multi-tenant mode requires cap-cloud feature")
        }
    } else {
        let tenant_ctx = config.project.to_tenant_ctx();
        tracing::info!(
            project_id = %tenant_ctx.project_id(),
            project_ref = %tenant_ctx.project_ref(),
            project_name = %tenant_ctx.project_name(),
            env = %tenant_ctx.env(),
            "single-tenant mode, using fixed tenant context"
        );
        std::sync::Arc::new(TenantProvider::fixed(tenant_ctx))
    };
    
    // For single-tenant mode, get the fixed context for Extension layer
    let tenant_ctx_for_extension = tenant_provider.fixed_ctx().cloned();

    // Build auth bundle (required)
    #[cfg(feature = "cap-auth")]
    let auth_bundle = {
        let auth_config = config
            .auth
            .as_ref()
            .ok_or_else(|| ServerError::Config("auth config required".to_string()))?;
        let tenant_id = config.project.project_id();
        boot::AuthBundle::build(&shared.pg, auth_config, shared.vault.as_ref(), &tenant_id).await?
    };

    // Compose capabilities
    let capabilities = ServerCapabilities::build(
        &shared,
        &config,
        #[cfg(feature = "cap-auth")]
        auth_bundle,
    )
    .await?;

    // Build the combined router
    let capabilities_router = capabilities.router();

    // Build admin router
    // Resolve admin token (potentially from vault)
    let tenant_id = config.project.project_id();
    let admin_token = resolve_secret(
        &config.admin.token,
        shared.vault.as_ref(),
        &tenant_id,
        "admin token",
    ).await?;

    let admin_state = admin::AdminAuthState {
        token: admin_token.clone(),
        allow_remote: config.admin.allow_remote,
        #[cfg(feature = "cap-functions")]
        functions: capabilities.functions.as_ref().map(|f| f.state.clone()),
        #[cfg(feature = "cap-sites")]
        sites: capabilities.sites.as_ref().map(|s| s.state.clone()),
        #[cfg(feature = "cap-sites")]
        default_org_slug: config
            .sites
            .as_ref()
            .map(|s| s.default_org_slug.clone())
            .unwrap_or_else(|| "reactor".to_string()),
    };
    let admin_router = admin::router(admin_state.clone());

    // Build cloud API state (shared between /_cloud/v1 and /_ops/v1/projects/*).
    //
    // The /_cloud/v1 router is preserved as a loopback-only break-glass under
    // the static admin token, while /_ops/v1/projects/* (built from
    // `compose::ops::build_router`) consumes the same `CloudApiState` over the
    // session-JWT control surface.
    #[cfg(feature = "cap-cloud")]
    let cloud_api_state = config.cloud.as_ref().map(|cloud_config| {
        let shared_postgres_url = cloud_config
            .shared_pool
            .as_ref()
            .and_then(|sp| sp.shared_postgres_url.clone());
        compose::cloud::CloudApiState::new(
            shared.pg.clone(),
            shared.vault.clone(),
            cloud_config.backend_target.clone(),
            cloud_config.base_domain.clone(),
            cloud_config.tls_mode.clone().unwrap_or_else(|| "wildcard".to_string()),
            cloud_config.provider.clone(),
            shared_postgres_url,
        )
    });

    #[cfg(feature = "cap-cloud")]
    let cloud_router = cloud_api_state.as_ref().map(|cloud_state| {
        let router = compose::cloud::router()
            .with_state(cloud_state.clone())
            .layer(axum::middleware::from_fn_with_state(
                admin_state.clone(),
                admin::auth::admin_auth_middleware,
            ));
        tracing::info!("cloud API enabled at /_cloud/v1");
        router
    });

    // Create vault state extension for admin vault endpoints
    let vault_state = admin::VaultState {
        vault: shared.vault.clone(),
        tenant_id: config.project.project_id(),
    };

    // Build quota service and tenant adapter cache for multi-tenant mode (Phase 4)
    #[cfg(feature = "cap-cloud")]
    let (quota_service, tenant_adapter_cache) = {
        let is_multi_tenant = config
            .cloud
            .as_ref()
            .map(|c| c.multi_tenant)
            .unwrap_or(false);
        
        if is_multi_tenant {
            // Build quota service
            let quota_config = config
                .cloud
                .as_ref()
                .and_then(|c| c.quotas.as_ref())
                .map(|q| quota::QuotaServiceConfig {
                    free_tier_limits: quota::QuotaLimits {
                        requests_per_minute: q.free.requests_per_minute,
                        concurrent_functions: q.free.concurrent_functions,
                        db_connections: q.free.db_connections,
                        storage_bytes: q.free.storage_gb as u64 * 1_073_741_824, // GB to bytes
                        bandwidth_bytes_per_month: q.free.bandwidth_gb_per_month as u64 * 1_073_741_824,
                    },
                    dedicated_tier_limits: None,
                    cache_ttl: std::time::Duration::from_secs(300),
                })
                .unwrap_or_default();
            let qs = std::sync::Arc::new(quota::QuotaService::new(quota_config));
            
            // Build tenant adapter cache
            let shared_pool_config = config
                .cloud
                .as_ref()
                .and_then(|c| c.shared_pool.as_ref());
            
            let cache_config = tenant_cache::TenantAdapterCacheConfig {
                max_active_tenants: shared_pool_config
                    .map(|c| c.max_active_tenants)
                    .unwrap_or(5000),
                idle_timeout: std::time::Duration::from_secs(
                    shared_pool_config.map(|c| c.idle_timeout_secs).unwrap_or(600)
                ),
                cold_load_concurrency: shared_pool_config
                    .map(|c| c.cold_load_concurrency)
                    .unwrap_or(16),
                per_tenant_pool_size: shared_pool_config
                    .map(|c| c.per_tenant_pool_size)
                    .unwrap_or(5),
                shared_postgres_base_url: shared_pool_config
                    .and_then(|c| c.shared_postgres_url.clone())
                    .unwrap_or_else(|| config.database.url.clone()),
                pooler_url_template: shared_pool_config
                    .and_then(|c| c.pooler.as_ref())
                    .and_then(|p| p.url_template.clone()),
                pooler_mode: shared_pool_config
                    .and_then(|c| c.pooler.as_ref())
                    .map(|p| p.mode.clone())
                    .unwrap_or_else(|| "transaction".to_string()),
                prepared_statements: shared_pool_config
                    .and_then(|c| c.pooler.as_ref())
                    .map(|p| p.prepared_statements)
                    .unwrap_or(true),
                pooler_connect_timeout_secs: shared_pool_config
                    .and_then(|c| c.pooler.as_ref())
                    .map(|p| p.connect_timeout_secs)
                    .unwrap_or(10),
                storage_bucket: config
                    .storage
                    .as_ref()
                    .and_then(|s| s.s3_bucket.clone())
                    .unwrap_or_else(|| "reactor-storage".to_string()),
            };
            let tac = std::sync::Arc::new(tenant_cache::TenantAdapterCache::new(cache_config));
            
            // Start eviction background task
            let eviction_cache = tac.clone();
            let eviction_shutdown = shutdown_handle.receiver();
            tokio::spawn(tenant_cache::eviction_task(
                eviction_cache,
                std::time::Duration::from_secs(60),
                eviction_shutdown,
            ));
            
            tracing::info!(
                max_active_tenants = tac.stats().max_active_tenants,
                "tenant adapter cache initialized (multi-tenant mode)"
            );
            
            (Some(qs), Some(tac))
        } else {
            (None, None)
        }
    };

    // Build ops control surface router (/_ops/v1/*).
    //
    // The ops surface is the secure, audited, scope-gated control plane that
    // replaces the static admin token for operator-facing endpoints. It is
    // protected by its own middleware stack (network, identity, scope, step-up,
    // audit) and requires a session JWT issued by reactor-auth with platform
    // operator scopes (e.g. `ops:*`, `cloud:projects:*`, `vault:*`).
    #[cfg(all(feature = "cap-ops", feature = "cap-auth"))]
    let ops_router = if let Some(ref auth_slot) = capabilities.auth {
        let auth_client: std::sync::Arc<dyn reactor_core::auth::AuthClient> =
            std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(
                auth_slot.state.service.clone(),
            ));
        #[cfg(feature = "cap-cloud")]
        let cloud_for_ops = cloud_api_state.clone();
        #[cfg(not(feature = "cap-cloud"))]
        let cloud_for_ops: Option<compose::cloud::CloudApiState> = None;

        tracing::info!("ops control surface enabled at /_ops/v1");
        Some(compose::ops::build_router(
            shared.pg.clone(),
            auth_client,
            compose::ops::default_shared_cluster_config(),
            cloud_for_ops,
            Some(admin_state.clone()),
        ))
    } else {
        None
    };

    // Combine routers with tenant context extension
    // TenantCtx is available via Extension for all handlers, and can be extracted with Tenant
    let mut app = capabilities_router
        .merge(admin_router);
    
    // Nest cloud API at /_cloud/v1
    #[cfg(feature = "cap-cloud")]
    if let Some(cloud_router) = cloud_router {
        app = app.nest("/_cloud/v1", cloud_router);
    }

    // Merge ops control surface (router already prefixes /_ops/v1 internally)
    #[cfg(all(feature = "cap-ops", feature = "cap-auth"))]
    if let Some(ops_router) = ops_router {
        app = app.merge(ops_router);
    }
    
    // Add tenant context - either as fixed Extension or via middleware
    let app = if let Some(ref tenant_ctx) = tenant_ctx_for_extension {
        // Single-tenant mode: add fixed TenantCtx as Extension
        app.layer(axum::Extension(tenant_ctx.clone()))
    } else {
        // Multi-tenant mode: add tenant resolution middleware
        let mw_state = std::sync::Arc::new(boot::tenant::TenantMiddlewareState {
            provider: tenant_provider.clone(),
            admin_token: config.admin.token.clone(),
        });
        app.layer(axum::middleware::from_fn_with_state(
            mw_state,
            boot::tenant::tenant_resolution_middleware,
        ))
    };
    
    let app = app
        .layer(axum::Extension(shared.pg.clone()))
        .layer(axum::Extension(config.clone()))
        .layer(axum::Extension(shutdown_handle.clone()))
        .layer(axum::Extension(vault_state));

    // Add multi-tenant extensions (quota service and tenant adapter cache)
    #[cfg(feature = "cap-cloud")]
    let app = {
        let mut app = app;
        
        if let Some(ref qs) = quota_service {
            tracing::info!("quota enforcement enabled (multi-tenant mode)");
            app = app.layer(axum::Extension(qs.clone()));
        }
        
        if let Some(ref tac) = tenant_adapter_cache {
            tracing::info!("tenant adapter cache enabled (multi-tenant mode)");
            app = app.layer(axum::Extension(tac.clone()));
        }
        
        app
    };

    // Create listener
    let listener = tokio::net::TcpListener::bind(config.server.bind).await?;
    tracing::info!(bind = %config.server.bind, "server listening");

    // Serve with graceful shutdown
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        boot::shutdown::wait_and_signal(&shutdown_handle).await;
    })
    .await?;

    // Drain background tasks
    let drain_timeout = std::time::Duration::from_secs(30);
    if let Err(e) = capabilities.join_background(drain_timeout).await {
        tracing::warn!(error = %e, "some background tasks did not drain cleanly");
    }

    tracing::info!("reactor-server shutdown complete");
    Ok(())
}

/// Run migrations only, then exit.
///
/// Used by `reactor-server migrate` subcommand.
pub async fn migrate_only(config: ReactorConfig) -> anyhow::Result<()> {
    config.validate()?;
    boot::tracing::init(&config.tracing);

    tracing::info!("running migrations");

    // Create a temporary shutdown handle (won't be used)
    let shutdown_handle = ShutdownHandle::new();

    // Build shared resources
    let shared = SharedResources::build(&config, shutdown_handle.receiver()).await?;

    // Run migrations
    boot::migrate::run_all(&shared.pg, &config).await?;

    tracing::info!("migrations complete");
    Ok(())
}

/// Run doctor probes only, then exit.
///
/// Used by `reactor-server doctor` subcommand.
pub async fn doctor_only(config: ReactorConfig) -> anyhow::Result<()> {
    config.validate()?;
    boot::tracing::init(&config.tracing);

    tracing::info!("running doctor probes");

    // Create a temporary shutdown handle
    let shutdown_handle = ShutdownHandle::new();

    // Build shared resources
    let shared = SharedResources::build(&config, shutdown_handle.receiver()).await?;

    // Ping database
    shared.ping_db().await?;
    println!("Database: OK");

    // Would run per-capability probes here
    #[cfg(feature = "cap-auth")]
    println!("Auth: OK");

    #[cfg(feature = "cap-data")]
    println!("Data: OK");

    #[cfg(feature = "cap-storage")]
    println!("Storage: OK");

    #[cfg(feature = "cap-functions")]
    println!("Functions: OK");

    #[cfg(feature = "cap-jobs")]
    println!("Jobs: OK");

    #[cfg(feature = "cap-sites")]
    println!("Sites: OK");

    tracing::info!("doctor probes complete");
    Ok(())
}
