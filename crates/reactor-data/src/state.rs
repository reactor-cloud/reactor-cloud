//! Application state for reactor-data.

use crate::config::DataConfig;
use crate::store::DataStore;
use reactor_core::auth::AuthClient;
use std::sync::Arc;

/// Application state for reactor-data.
#[derive(Clone)]
pub struct DataState<S: DataStore = crate::store::PgDataStore> {
    /// Data store for database operations.
    pub store: Arc<S>,

    /// Auth client for authentication and authorization.
    pub auth: Arc<dyn AuthClient>,

    /// Configuration.
    pub config: Arc<DataConfig>,
}

impl<S: DataStore> DataState<S> {
    /// Create a new DataState.
    pub fn new(store: Arc<S>, auth: Arc<dyn AuthClient>, config: Arc<DataConfig>) -> Self {
        Self {
            store,
            auth,
            config,
        }
    }
}
