//! State for the ops control surface.

use crate::config::OpsConfig;
use reactor_core::auth::AuthClient;
use sqlx::PgPool;
use std::sync::Arc;

/// Shared state for the ops control surface.
#[derive(Clone)]
pub struct OpsState {
    /// Auth client for identity verification.
    pub auth: Arc<dyn AuthClient>,
    /// Database pool for audit logging.
    pub pool: PgPool,
    /// Ops configuration.
    pub config: Arc<OpsConfig>,
}

impl OpsState {
    /// Create a new ops state.
    pub fn new(
        auth: Arc<dyn AuthClient>,
        pool: PgPool,
        config: OpsConfig,
    ) -> Self {
        Self {
            auth,
            pool,
            config: Arc::new(config),
        }
    }
}
