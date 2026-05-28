//! Jobs state types.

use reactor_cache::PostgresBackend;
use reactor_core::auth::{AuthClient, AuthCtx};
use reactor_core::id::{OrgId, UserId};
use sqlx::PgPool;
use std::sync::Arc;

use crate::config::JobsConfig;

/// Request-scoped job context.
#[derive(Debug, Clone)]
pub struct JobCtx {
    /// Auth context from middleware.
    pub auth: AuthCtx,
    /// Request ID for tracing.
    pub request_id: String,
    /// Active organization (required for jobs).
    org_id: OrgId,
}

impl JobCtx {
    /// Create a new job context.
    pub fn new(auth: AuthCtx, request_id: String, org_id: OrgId) -> Self {
        Self { auth, request_id, org_id }
    }

    /// Get the user ID if authenticated.
    pub fn user_id(&self) -> Option<UserId> {
        self.auth.user_id()
    }

    /// Get the active organization ID.
    pub fn active_org(&self) -> &OrgId {
        &self.org_id
    }

    /// Check if the context has a specific permission.
    pub fn has_permission(&self, perm: &str) -> bool {
        self.auth.has_permission(perm)
    }
}

/// Shared application state.
#[derive(Clone)]
pub struct JobsState {
    /// Database connection pool.
    pub pool: PgPool,
    /// Configuration.
    pub config: Arc<JobsConfig>,
    /// Auth client.
    pub auth: Arc<dyn AuthClient>,
    /// Cache backend.
    pub cache: Arc<PostgresBackend>,
    /// HTTP client for calling reactor-functions.
    pub http_client: reqwest::Client,
}

impl JobsState {
    /// Create new jobs state.
    pub fn new(
        pool: PgPool,
        config: Arc<JobsConfig>,
        auth: Arc<dyn AuthClient>,
        cache: Arc<PostgresBackend>,
    ) -> Self {
        let http_client = reqwest::Client::new();
        Self {
            pool,
            config,
            auth,
            cache,
            http_client,
        }
    }
}
