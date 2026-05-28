//! Functions service state.

use crate::config::FunctionsConfig;
use crate::routes::FunctionMetrics;
use crate::runtime::RuntimeRegistry;
use reactor_core::auth::{AuthClient, AuthCtx};
use reactor_core::id::{OrgId, UserId};
use sqlx::PgPool;
use std::sync::Arc;

/// Functions service state.
#[derive(Clone)]
pub struct FunctionsState {
    /// Database connection pool.
    pub pool: PgPool,

    /// Functions configuration.
    pub config: Arc<FunctionsConfig>,

    /// Authentication client.
    pub auth: Arc<dyn AuthClient>,

    /// Runtime registry for managing function runtimes.
    pub runtimes: Arc<RuntimeRegistry>,

    /// Prometheus metrics.
    pub metrics: Arc<FunctionMetrics>,
}

impl FunctionsState {
    /// Create a new functions state.
    pub fn new(
        pool: PgPool,
        config: Arc<FunctionsConfig>,
        auth: Arc<dyn AuthClient>,
        runtimes: Arc<RuntimeRegistry>,
    ) -> Self {
        Self {
            pool,
            config,
            auth,
            runtimes,
            metrics: Arc::new(FunctionMetrics::new()),
        }
    }
}

/// Request-local context for function operations.
#[derive(Debug, Clone)]
pub struct FunctionCtx {
    /// Authentication context from the resolved token.
    pub auth: AuthCtx,

    /// Request ID for tracing.
    pub request_id: String,

    /// Active organization ID (required for all operations).
    pub org_id: OrgId,
}

impl FunctionCtx {
    /// Create a new function context from an auth context.
    ///
    /// Returns an error if no active org is present.
    pub fn from_auth(auth: AuthCtx, request_id: String) -> Result<Self, crate::FunctionsError> {
        let org_id = auth.active_org.ok_or(crate::FunctionsError::OrgRequired)?;

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
