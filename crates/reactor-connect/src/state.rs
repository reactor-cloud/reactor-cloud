//! Connect service state types.

use crate::config::ConnectConfig;
use crate::runtime::ConnectorRuntime;
use crate::store::{ConnectStore, OrgId};
use reactor_cache::CacheBackend;
use reactor_core::auth::AuthClient;
use reactor_core::Vault;
use std::sync::Arc;
use uuid::Uuid;

/// Shared state for the Connect service.
#[derive(Clone)]
pub struct ConnectState<S: ConnectStore + Clone + 'static> {
    /// Database store.
    pub store: S,

    /// Auth client for token verification.
    pub auth: Arc<dyn AuthClient>,

    /// Vault for credential storage.
    pub vault: Arc<dyn Vault>,

    /// Cache backend for idempotency keys and rate limiting.
    pub cache: Arc<dyn CacheBackend>,

    /// Connector runtime.
    pub runtime: Arc<dyn ConnectorRuntime>,

    /// Service configuration.
    pub config: Arc<ConnectConfig>,

    /// HTTP client for downstream service calls.
    pub http_client: reqwest::Client,
}

impl<S: ConnectStore + Clone + 'static> ConnectState<S> {
    /// Create a new Connect state.
    pub fn new(
        store: S,
        auth: Arc<dyn AuthClient>,
        vault: Arc<dyn Vault>,
        cache: Arc<dyn CacheBackend>,
        runtime: Arc<dyn ConnectorRuntime>,
        config: ConnectConfig,
    ) -> Self {
        Self {
            store,
            auth,
            vault,
            cache,
            runtime,
            config: Arc::new(config),
            http_client: reqwest::Client::new(),
        }
    }
}

/// Request-local context for Connect operations.
#[derive(Debug, Clone)]
pub struct ConnectCtx {
    /// Request ID for tracing.
    pub request_id: String,

    /// Active organization ID.
    pub org_id: OrgId,

    /// User ID if authenticated.
    pub user_id: Option<Uuid>,
}

impl ConnectCtx {
    /// Create a new Connect context.
    pub fn new(request_id: String, org_id: OrgId, user_id: Option<Uuid>) -> Self {
        Self {
            request_id,
            org_id,
            user_id,
        }
    }

    /// Get the user ID if present.
    pub fn user_id(&self) -> Option<Uuid> {
        self.user_id
    }

    /// Get the active organization ID.
    pub fn active_org(&self) -> &OrgId {
        &self.org_id
    }
}
