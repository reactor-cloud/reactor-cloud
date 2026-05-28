//! Cloud control plane boot module.
//!
//! Handles startup tasks for the cloud control plane, including:
//! - Resuming projects stuck in 'provisioning' state
//! - Resuming teardown for projects in 'deleting' state

use std::sync::Arc;

use reactor_cloud_api::{
    CloudProvider, PgProjectStore, ProjectService, ProjectStore, SingleNodeConfig,
    SingleNodeProvider,
};
use reactor_core::primitives::vault::Vault;
use sqlx::PgPool;

/// Cloud bootstrap configuration.
pub struct CloudBootstrapConfig {
    /// Base domain for project subdomains (e.g., "reactor.cloud").
    pub base_domain: String,
    /// Backend target for gateway routing.
    pub backend_target: String,
    /// TLS mode for routes.
    pub tls_mode: String,
}

impl Default for CloudBootstrapConfig {
    fn default() -> Self {
        Self {
            base_domain: "reactor.local".to_string(),
            backend_target: "localhost:8000".to_string(),
            tls_mode: "none".to_string(),
        }
    }
}

/// Bootstrap the cloud control plane on server startup.
///
/// This function resumes any projects that were interrupted during provisioning
/// or teardown. It should be called after migrations but before the server
/// starts accepting requests.
pub async fn bootstrap(
    pool: &PgPool,
    vault: Arc<dyn Vault>,
    config: &CloudBootstrapConfig,
) -> Result<CloudBootstrapResult, CloudBootstrapError> {
    tracing::info!("starting cloud control plane bootstrap");

    // Create store
    let store: Arc<dyn ProjectStore> = Arc::new(PgProjectStore::new(pool.clone()));

    // Create provider
    let provider_config = SingleNodeConfig {
        backend_target: config.backend_target.clone(),
        base_domain: config.base_domain.clone(),
        tls_mode: config.tls_mode.clone(),
    };
    let provider: Arc<dyn CloudProvider> =
        Arc::new(SingleNodeProvider::new(pool.clone(), vault.clone(), store.clone(), provider_config));

    // Create project service
    let project_service = ProjectService::new(store.clone(), provider);

    // Resume provisioning for any stuck projects
    let provisioning_resumed = project_service
        .resume_provisioning()
        .await
        .map_err(|e| CloudBootstrapError::ResumeProvisioning(e.to_string()))?;

    if provisioning_resumed > 0 {
        tracing::info!(count = provisioning_resumed, "resumed provisioning for projects");
    }

    // Resume teardown for any stuck projects
    let teardown_resumed = project_service
        .resume_teardown()
        .await
        .map_err(|e| CloudBootstrapError::ResumeTeardown(e.to_string()))?;

    if teardown_resumed > 0 {
        tracing::info!(count = teardown_resumed, "resumed teardown for projects");
    }

    Ok(CloudBootstrapResult {
        provisioning_resumed,
        teardown_resumed,
    })
}

/// Result of cloud bootstrap.
#[derive(Debug)]
pub struct CloudBootstrapResult {
    /// Number of projects resumed from provisioning state.
    pub provisioning_resumed: usize,
    /// Number of projects resumed from deleting state.
    pub teardown_resumed: usize,
}

/// Error during cloud bootstrap.
#[derive(Debug, thiserror::Error)]
pub enum CloudBootstrapError {
    /// Failed to resume provisioning.
    #[error("failed to resume provisioning: {0}")]
    ResumeProvisioning(String),
    /// Failed to resume teardown.
    #[error("failed to resume teardown: {0}")]
    ResumeTeardown(String),
}
