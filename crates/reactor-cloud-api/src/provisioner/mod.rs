//! Cloud provisioner module.
//!
//! Defines the [`CloudProvider`] trait and implementations for provisioning
//! tenant infrastructure.

pub mod shared_cluster;
pub mod single_node;

pub use shared_cluster::{SharedClusterConfig, SharedClusterProvider};
pub use single_node::{SingleNodeConfig, SingleNodeProvider};

use async_trait::async_trait;

use crate::error::{HealthError, ProvisionError, TeardownError};
use crate::types::{BackendKind, ProjectHealth, ProjectSpec, ProvisionResult};
use reactor_core::ProjectId;

/// Trait for cloud infrastructure providers.
///
/// A `CloudProvider` knows how to provision and tear down tenant infrastructure.
/// Different implementations handle different deployment topologies:
///
/// - `SingleNodeProvider`: Phase 3, dedicated single-node (Fly machine)
/// - `SharedClusterProvider`: Phase 4+, shared k8s cluster
/// - `FlyDedicatedProvider`: Phase 5+, dedicated Fly machines per tenant
/// - `ByocProvider`: Phase 5+, bring-your-own-cloud
#[async_trait]
pub trait CloudProvider: Send + Sync {
    /// Returns the backend kind this provider creates.
    fn backend_kind(&self) -> BackendKind;

    /// Provision infrastructure for a new project.
    ///
    /// This should:
    /// 1. Create the tenant schema
    /// 2. Run capability migrations
    /// 3. Bootstrap vault keys
    /// 4. Configure gateway routing
    ///
    /// Returns the backend target and initial API keys on success.
    async fn provision(&self, spec: &ProjectSpec) -> Result<ProvisionResult, ProvisionError>;

    /// Tear down infrastructure for a project.
    ///
    /// This should:
    /// 1. Remove gateway routing
    /// 2. Delete vault paths
    /// 3. Drop the tenant schema
    async fn teardown(&self, project_id: &ProjectId) -> Result<(), TeardownError>;

    /// Check the health of a project's infrastructure.
    async fn health(&self, project_id: &ProjectId) -> Result<ProjectHealth, HealthError>;
}
