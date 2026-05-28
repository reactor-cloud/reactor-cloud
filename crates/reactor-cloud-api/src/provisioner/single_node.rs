//! Single-node provider for Phase 3 deployments.
//!
//! This provider provisions tenant infrastructure on the same machine:
//! - Creates tenant schema in Postgres
//! - Bootstraps vault keys
//! - Configures gateway routing

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

use crate::bootstrap::{BootstrapConfig, SchemaBootstrap, VaultBootstrap};
use crate::error::{HealthError, ProvisionError, TeardownError};
use crate::provisioner::CloudProvider;
use crate::store::ProjectStore;
use crate::types::{BackendKind, ProjectHealth, ProjectSpec, ProvisionResult};
use reactor_core::{ProjectId, Vault};

/// Single-node cloud provider for Phase 3.
///
/// Provisions all tenants on the same machine with isolated Postgres schemas
/// and vault paths. Gateway routes point to the local backend.
pub struct SingleNodeProvider {
    pool: PgPool,
    vault: Arc<dyn Vault>,
    store: Arc<dyn ProjectStore>,
    config: SingleNodeConfig,
}

/// Configuration for the single-node provider.
#[derive(Debug, Clone)]
pub struct SingleNodeConfig {
    /// Backend target for gateway routing (e.g., "reactor-cloud.internal:8000").
    pub backend_target: String,
    /// Base domain for project subdomains.
    pub base_domain: String,
    /// TLS mode for routes.
    pub tls_mode: String,
}

impl Default for SingleNodeConfig {
    fn default() -> Self {
        Self {
            backend_target: "reactor-cloud.internal:8000".to_string(),
            base_domain: "reactor.cloud".to_string(),
            tls_mode: "wildcard".to_string(),
        }
    }
}

impl SingleNodeProvider {
    /// Create a new single-node provider.
    pub fn new(
        pool: PgPool,
        vault: Arc<dyn Vault>,
        store: Arc<dyn ProjectStore>,
        config: SingleNodeConfig,
    ) -> Self {
        Self {
            pool,
            vault,
            store,
            config,
        }
    }

    /// Get the schema name for a project ref.
    fn schema_name(project_ref: &str) -> String {
        format!("tenant_{}", project_ref)
    }

    /// Get the hostname for a project ref.
    fn hostname(&self, project_ref: &str) -> String {
        format!("{}.{}", project_ref, self.config.base_domain)
    }
}

#[async_trait]
impl CloudProvider for SingleNodeProvider {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Dedicated
    }

    #[instrument(skip(self), fields(project_id = %spec.project_id, project_ref = %spec.project_ref))]
    async fn provision(&self, spec: &ProjectSpec) -> Result<ProvisionResult, ProvisionError> {
        info!("provisioning project");

        let schema = Self::schema_name(spec.project_ref.as_str());
        let project_id = spec.project_id;

        // Step 1: Create tenant schema
        debug!(schema = %schema, "creating tenant schema");
        SchemaBootstrap::create_schema(&self.pool, &schema)
            .await
            .map_err(|e| ProvisionError::SchemaCreation(e.to_string()))?;

        // Step 2: Run capability migrations
        debug!(schema = %schema, "running capability migrations");
        let bootstrap_config = BootstrapConfig::default();
        SchemaBootstrap::run_migrations(&self.pool, &schema, &bootstrap_config)
            .await
            .map_err(|e| ProvisionError::Migration(e.to_string()))?;

        // Step 3: Bootstrap vault keys
        debug!("bootstrapping vault keys");
        let vault_result = VaultBootstrap::bootstrap(&self.vault, &project_id)
            .await
            .map_err(|e| ProvisionError::VaultBootstrap(e.to_string()))?;

        // Step 4: Generate API keys (JWTs)
        debug!("generating API keys");
        let anon_key = VaultBootstrap::generate_anon_jwt(
            &self.vault,
            &project_id,
            spec.project_ref.as_str(),
        )
        .await
        .map_err(|e| ProvisionError::VaultBootstrap(format!("anon JWT: {}", e)))?;

        let service_key = VaultBootstrap::generate_service_jwt(
            &self.vault,
            &project_id,
            spec.project_ref.as_str(),
        )
        .await
        .map_err(|e| ProvisionError::VaultBootstrap(format!("service JWT: {}", e)))?;

        // Step 5: Create gateway route
        debug!("creating gateway route");
        let hostname = self.hostname(spec.project_ref.as_str());
        self.store
            .create_route(
                &hostname,
                &project_id,
                spec.project_ref.as_str(),
                "dedicated",
                &self.config.backend_target,
                &self.config.tls_mode,
            )
            .await
            .map_err(|e| ProvisionError::RouteCreation(e.to_string()))?;

        info!(
            hostname = %hostname,
            transit_keys = vault_result.transit_keys_created,
            "project provisioned successfully"
        );

        Ok(ProvisionResult {
            backend_target: self.config.backend_target.clone(),
            anon_key,
            service_key,
        })
    }

    #[instrument(skip(self), fields(project_id = %project_id))]
    async fn teardown(&self, project_id: &ProjectId) -> Result<(), TeardownError> {
        info!("tearing down project");

        // Get project info first
        let project = self
            .store
            .get_project_by_id(project_id)
            .await
            .map_err(|e| TeardownError::Database(e.to_string()))?
            .ok_or_else(|| TeardownError::Database("project not found".to_string()))?;

        let schema = Self::schema_name(&project.project_ref);
        let hostname = self.hostname(&project.project_ref);

        // Step 1: Remove gateway route
        debug!(hostname = %hostname, "removing gateway route");
        if let Err(e) = self.store.delete_route(&hostname).await {
            error!(error = %e, "failed to delete route (continuing)");
        }

        // Step 2: Delete vault secrets (keys persist for now - cleanup is idempotent)
        debug!("cleaning up vault secrets");
        if let Err(e) = VaultBootstrap::cleanup(&self.vault, project_id).await {
            error!(error = %e, "failed to cleanup vault (continuing)");
        }

        // Step 3: Drop tenant schema
        debug!(schema = %schema, "dropping tenant schema");
        SchemaBootstrap::drop_schema(&self.pool, &schema)
            .await
            .map_err(|e| TeardownError::SchemaDrop(e.to_string()))?;

        info!("project teardown complete");
        Ok(())
    }

    #[instrument(skip(self), fields(project_id = %project_id))]
    async fn health(&self, project_id: &ProjectId) -> Result<ProjectHealth, HealthError> {
        // Get project info
        let project = self
            .store
            .get_project_by_id(project_id)
            .await
            .map_err(|e| HealthError::DatabaseConnectivity(e.to_string()))?
            .ok_or(HealthError::SchemaNotFound)?;

        let schema = Self::schema_name(&project.project_ref);
        let hostname = self.hostname(&project.project_ref);

        // Check schema exists
        let schema_exists = SchemaBootstrap::schema_exists(&self.pool, &schema)
            .await
            .unwrap_or(false);

        // Check vault accessible
        let vault_accessible = self.vault.is_healthy().await;

        // Check route exists
        let route_configured = self
            .store
            .get_route(&hostname)
            .await
            .map(|r| r.is_some())
            .unwrap_or(false);

        let healthy = schema_exists && vault_accessible && route_configured;

        Ok(ProjectHealth {
            healthy,
            schema_exists,
            vault_accessible,
            route_configured,
            error: if healthy {
                None
            } else {
                Some(format!(
                    "schema={}, vault={}, route={}",
                    schema_exists, vault_accessible, route_configured
                ))
            },
        })
    }
}
