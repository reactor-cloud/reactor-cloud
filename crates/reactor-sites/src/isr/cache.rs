//! ISR cache implementation.

use crate::error::SitesError;
use crate::store::{IsrCacheEntry, PgSitesStore, SiteId, SitesStore};
use reactor_cache::{KvOperations, PostgresBackend};
use std::sync::Arc;
use std::time::Duration;

/// ISR cache with in-memory layer backed by Postgres.
pub struct IsrCache {
    cache: Arc<PostgresBackend>,
    store: Arc<PgSitesStore>,
}

impl IsrCache {
    /// Create a new ISR cache.
    pub fn new(cache: Arc<PostgresBackend>, store: Arc<PgSitesStore>) -> Self {
        Self { cache, store }
    }

    /// Get a cached entry.
    pub async fn get(
        &self,
        site_id: &SiteId,
        path: &str,
    ) -> Result<Option<IsrCacheEntry>, SitesError> {
        let cache_key = format!("isr:{}:{}", site_id, path);

        if let Some(data) = self.cache.get(&cache_key).await.ok().flatten() {
            if let Ok(entry) = serde_json::from_slice::<IsrCacheEntry>(&data) {
                return Ok(Some(entry));
            }
        }

        self.store.get_isr_entry(site_id, path).await
    }

    /// Set a cached entry.
    pub async fn set(&self, entry: &IsrCacheEntry, ttl: Option<Duration>) -> Result<(), SitesError> {
        let cache_key = format!("isr:{}:{}", entry.site_id, entry.path);

        let data = serde_json::to_vec(entry)
            .map_err(|e| SitesError::Internal(e.to_string()))?;

        self.cache
            .set(&cache_key, &data, ttl)
            .await
            .map_err(|e| SitesError::Internal(e.to_string()))?;

        self.store.set_isr_entry(entry).await
    }

    /// Invalidate cached entries by path or tag.
    pub async fn invalidate(
        &self,
        site_id: &SiteId,
        path_or_tag: &str,
    ) -> Result<u32, SitesError> {
        let cache_key = format!("isr:{}:{}", site_id, path_or_tag);
        let _ = self.cache.del(&cache_key).await;

        self.store.invalidate_isr(site_id, path_or_tag).await
    }
}
