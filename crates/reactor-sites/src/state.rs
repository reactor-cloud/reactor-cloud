//! Sites service state.

use crate::config::SitesConfig;
use crate::dispatch::{FunctionsClient, StorageClient};
use crate::routes::SiteMetrics;
use reactor_cache::PostgresBackend;
use reactor_core::auth::{AuthClient, AuthCtx};
use reactor_core::id::{OrgId, UserId};
use sqlx::PgPool;
use std::sync::Arc;

/// Sites service state.
#[derive(Clone)]
pub struct SitesState {
    /// Database connection pool.
    pub pool: PgPool,

    /// Sites configuration.
    pub config: Arc<SitesConfig>,

    /// Authentication client.
    pub auth: Arc<dyn AuthClient>,

    /// Functions service client.
    pub functions: Arc<FunctionsClient>,

    /// Storage service client.
    pub storage: Arc<StorageClient>,

    /// Cache backend for ISR.
    pub cache: Arc<PostgresBackend>,

    /// Prometheus metrics.
    pub metrics: Arc<SiteMetrics>,
}

impl SitesState {
    /// Create a new sites state.
    pub fn new(
        pool: PgPool,
        config: Arc<SitesConfig>,
        auth: Arc<dyn AuthClient>,
        functions: Arc<FunctionsClient>,
        storage: Arc<StorageClient>,
        cache: Arc<PostgresBackend>,
    ) -> Self {
        Self {
            pool,
            config,
            auth,
            functions,
            storage,
            cache,
            metrics: Arc::new(SiteMetrics::new()),
        }
    }
}

/// Request-local context for site admin operations.
#[derive(Debug, Clone)]
pub struct SiteCtx {
    /// Authentication context from the resolved token.
    pub auth: AuthCtx,

    /// Request ID for tracing.
    pub request_id: String,

    /// Active organization ID (required for all operations).
    pub org_id: OrgId,
}

impl SiteCtx {
    /// Create a new site context from an auth context.
    ///
    /// Returns an error if no active org is present.
    pub fn from_auth(auth: AuthCtx, request_id: String) -> Result<Self, crate::SitesError> {
        let org_id = auth.active_org.ok_or(crate::SitesError::OrgRequired)?;

        Ok(Self {
            auth,
            request_id,
            org_id,
        })
    }

    /// Get the user ID from the auth context.
    ///
    /// Returns `None` if this is an API key token.
    pub fn user_id(&self) -> Option<UserId> {
        self.auth.user_id()
    }

    /// Get the active organization ID as a Uuid for database operations.
    pub fn active_org(&self) -> uuid::Uuid {
        self.org_id.into()
    }

    /// Get the active organization ID as an OrgId.
    pub fn active_org_id(&self) -> OrgId {
        self.org_id
    }

    /// Check if the context has a specific permission.
    pub fn has_permission(&self, permission: &str) -> bool {
        self.auth.has_permission(permission)
    }
}
