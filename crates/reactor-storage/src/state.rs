//! Storage service state.

use crate::config::StorageConfig;
use crate::store::BlobStore;
use reactor_core::auth::AuthClient;
use sqlx::PgPool;
use std::sync::Arc;

/// Storage service state.
#[derive(Clone)]
pub struct StorageState {
    /// Database connection pool.
    pub pool: PgPool,

    /// Storage configuration.
    pub config: Arc<StorageConfig>,

    /// Authentication client.
    pub auth: Arc<dyn AuthClient>,

    /// Blob store for actual object storage (S3 or FS).
    /// Initialized during composition based on config.
    #[cfg(any(feature = "fs", feature = "s3"))]
    pub blob_store: Option<Arc<dyn BlobStore>>,
}

impl StorageState {
    /// Create a new storage state.
    pub fn new(pool: PgPool, config: Arc<StorageConfig>, auth: Arc<dyn AuthClient>) -> Self {
        Self {
            pool,
            config,
            auth,
            #[cfg(any(feature = "fs", feature = "s3"))]
            blob_store: None,
        }
    }

    /// Create a new storage state with a blob store.
    #[cfg(any(feature = "fs", feature = "s3"))]
    pub fn with_blob_store(
        pool: PgPool,
        config: Arc<StorageConfig>,
        auth: Arc<dyn AuthClient>,
        blob_store: Arc<dyn BlobStore>,
    ) -> Self {
        Self {
            pool,
            config,
            auth,
            blob_store: Some(blob_store),
        }
    }
}
