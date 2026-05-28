//! Key-value operations trait.

use async_trait::async_trait;
use std::time::Duration;

use crate::CacheError;

/// Key-value operations.
#[async_trait]
pub trait KvOperations: Send + Sync {
    /// Get a value by key.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;

    /// Set a value with optional TTL.
    async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<(), CacheError>;

    /// Delete a key.
    ///
    /// Returns true if the key existed.
    async fn del(&self, key: &str) -> Result<bool, CacheError>;

    /// Update the TTL of a key.
    ///
    /// Returns true if the key existed.
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool, CacheError>;

    /// Check if a key exists.
    async fn exists(&self, key: &str) -> Result<bool, CacheError>;
}
