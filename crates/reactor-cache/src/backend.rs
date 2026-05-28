//! Combined cache backend trait.

use async_trait::async_trait;

use crate::error::CacheError;
use crate::kv::KvOperations;
use crate::queue::QueueOperations;

/// Combined cache backend providing both queue and KV operations.
#[async_trait]
pub trait CacheBackend: QueueOperations + KvOperations + Send + Sync + 'static {
    /// Run any pending migrations for the backend.
    async fn migrate(&self) -> Result<(), CacheError>;

    /// Health check.
    async fn health_check(&self) -> Result<(), CacheError>;
}

/// Blanket implementation for types that implement both traits.
#[async_trait]
impl<T> CacheBackend for T
where
    T: QueueOperations + KvOperations + Send + Sync + 'static,
{
    async fn migrate(&self) -> Result<(), CacheError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<(), CacheError> {
        Ok(())
    }
}
