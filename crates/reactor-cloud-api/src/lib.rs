//! Cloud control plane API for Reactor.
//!
//! This crate provides the headless control plane for managing multi-tenant
//! Reactor projects. It includes:
//!
//! - [`projects`] — Project CRUD and lifecycle management
//! - [`members`] — Project membership and roles
//! - [`keys`] — API key issuance and rotation
//! - [`audit`] — Audit log access
//! - [`provisioner`] — Infrastructure provisioning (CloudProvider trait)
//! - [`bootstrap`] — Tenant schema and vault initialization
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                           reactor-cloud-api                             │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  Services                                                               │
//! │  ┌─────────────┬─────────────┬─────────────┬─────────────┐             │
//! │  │ ProjectSvc  │ MemberSvc   │ KeyService  │ AuditSvc    │             │
//! │  └──────┬──────┴──────┬──────┴──────┬──────┴──────┬──────┘             │
//! │         │             │             │             │                     │
//! │         ▼             ▼             ▼             ▼                     │
//! │  ┌──────────────────────────────────────────────────────┐              │
//! │  │              ProjectStore (PostgreSQL)               │              │
//! │  └──────────────────────────────────────────────────────┘              │
//! │                                                                         │
//! │  Provisioning                                                           │
//! │  ┌─────────────────────────────────────────────────────────────────┐   │
//! │  │                    CloudProvider trait                          │   │
//! │  │  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │   │
//! │  │  │SingleNodeProvider│ │SharedClusterProv│  │FlyDedicatedProv│  │   │
//! │  │  │    (Phase 3)    │  │    (Phase 4)   │  │   (Phase 5+)   │  │   │
//! │  │  └─────────────────┘  └─────────────────┘  └────────────────┘  │   │
//! │  └─────────────────────────────────────────────────────────────────┘   │
//! │                                                                         │
//! │  Bootstrap                                                              │
//! │  ┌─────────────────┐  ┌─────────────────┐                              │
//! │  │ SchemaBootstrap │  │ VaultBootstrap  │                              │
//! │  │ (tenant schema) │  │ (keys, secrets) │                              │
//! │  └─────────────────┘  └─────────────────┘                              │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use reactor_cloud_api::{
//!     ProjectService, MemberService, KeyService, AuditService,
//!     PgProjectStore, SingleNodeProvider, SingleNodeConfig,
//! };
//!
//! // Create the store
//! let store = Arc::new(PgProjectStore::new(pool.clone()));
//!
//! // Create the provider
//! let provider = Arc::new(SingleNodeProvider::new(
//!     pool.clone(),
//!     vault.clone(),
//!     store.clone(),
//!     SingleNodeConfig::default(),
//! ));
//!
//! // Create services
//! let projects = ProjectService::new(store.clone(), provider);
//! let members = MemberService::new(store.clone());
//! let keys = KeyService::new(store.clone(), vault);
//! let audit = AuditService::new(store);
//!
//! // Create a project
//! let result = projects.create(CreateProjectRequest {
//!     name: "My Project".to_string(),
//!     region: None,
//!     owner_user_id: user_id,
//! }).await?;
//!
//! println!("Project created: {}", result.project.project_ref);
//! println!("Anon key: {}", result.anon_key);
//! println!("Service key: {}", result.service_key);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod bootstrap;
pub mod error;
pub mod keys;
pub mod members;
pub mod projects;
pub mod provisioner;
pub mod store;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
pub mod types;

// Re-exports for convenience
pub use audit::AuditService;
pub use error::{CloudError, CloudResult};
pub use keys::{KeyResult, KeyService};
pub use members::MemberService;
pub use projects::{CreateProjectRequest, CreateProjectResult, ProjectService};
pub use provisioner::{
    CloudProvider, SharedClusterConfig, SharedClusterProvider, SingleNodeConfig, SingleNodeProvider,
};
pub use store::{PgProjectStore, ProjectStore, RouteInfo};
pub use types::{
    AuditAction, AuditEntry, BackendKind, KeyKind, MemberRole, Project, ProjectHealth, ProjectKey,
    ProjectMember, ProjectSpec, ProjectStatus, ProvisionResult,
};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
