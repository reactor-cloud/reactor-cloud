//! Error types for the cloud control plane.

use thiserror::Error;
use uuid::Uuid;

/// Result type for cloud operations.
pub type CloudResult<T> = Result<T, CloudError>;

/// Error type for cloud control plane operations.
#[derive(Debug, Error)]
pub enum CloudError {
    /// Project not found.
    #[error("project not found: {0}")]
    ProjectNotFound(String),

    /// Project already exists with the given ref.
    #[error("project with ref '{0}' already exists")]
    ProjectAlreadyExists(String),

    /// Member not found.
    #[error("member not found: project={project_id}, user={user_id}")]
    MemberNotFound {
        /// The project ID.
        project_id: Uuid,
        /// The user ID.
        user_id: Uuid,
    },

    /// Key not found.
    #[error("key not found: {0}")]
    KeyNotFound(Uuid),

    /// Invalid project status transition.
    #[error("invalid status transition from '{from}' to '{to}'")]
    InvalidStatusTransition {
        /// Current status.
        from: String,
        /// Target status.
        to: String,
    },

    /// Provisioning failed.
    #[error("provisioning failed: {0}")]
    ProvisioningFailed(String),

    /// Teardown failed.
    #[error("teardown failed: {0}")]
    TeardownFailed(String),

    /// Schema creation failed.
    #[error("schema creation failed: {0}")]
    SchemaCreationFailed(String),

    /// Migration failed.
    #[error("migration failed: {0}")]
    MigrationFailed(String),

    /// Vault operation failed.
    #[error("vault error: {0}")]
    Vault(String),

    /// Route configuration failed.
    #[error("route configuration failed: {0}")]
    RouteConfigFailed(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Invalid argument.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Error type for provisioning operations.
#[derive(Debug, Error)]
pub enum ProvisionError {
    /// Schema already exists.
    #[error("schema already exists: {0}")]
    SchemaExists(String),

    /// Schema creation failed.
    #[error("schema creation failed: {0}")]
    SchemaCreation(String),

    /// Database setup failed (CREATE DATABASE, CREATE ROLE, etc.).
    #[error("database setup failed: {0}")]
    DatabaseSetup(String),

    /// Migration failed.
    #[error("migration failed: {0}")]
    Migration(String),

    /// Vault bootstrap failed.
    #[error("vault bootstrap failed: {0}")]
    VaultBootstrap(String),

    /// Route creation failed.
    #[error("route creation failed: {0}")]
    RouteCreation(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(String),

    /// Invalid argument.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Error type for teardown operations.
#[derive(Debug, Error)]
pub enum TeardownError {
    /// Project is not in a deletable state.
    #[error("project is not deletable: current status is '{0}'")]
    NotDeletable(String),

    /// Schema drop failed.
    #[error("schema drop failed: {0}")]
    SchemaDrop(String),

    /// Vault cleanup failed.
    #[error("vault cleanup failed: {0}")]
    VaultCleanup(String),

    /// Route removal failed.
    #[error("route removal failed: {0}")]
    RouteRemoval(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(String),
}

/// Error type for health check operations.
#[derive(Debug, Error)]
pub enum HealthError {
    /// Schema not found.
    #[error("schema not found")]
    SchemaNotFound,

    /// Database connectivity failed.
    #[error("database connectivity failed: {0}")]
    DatabaseConnectivity(String),

    /// Vault connectivity failed.
    #[error("vault connectivity failed: {0}")]
    VaultConnectivity(String),
}
