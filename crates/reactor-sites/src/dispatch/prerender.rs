//! Prerender/ISR dispatch logic.

use crate::dispatch::RouteDecision;
use crate::error::SitesError;
use crate::store::{IsrCacheEntry, SiteId, SitesStore};
use chrono::Utc;
use std::sync::Arc;

/// ISR cache status for response headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    /// Cache hit (fresh content).
    Hit,
    /// Cache miss (had to render).
    Miss,
    /// Cache hit but content is stale (served stale, revalidating in background).
    Stale,
    /// Cache bypassed.
    Bypass,
}

impl std::fmt::Display for CacheStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheStatus::Hit => write!(f, "HIT"),
            CacheStatus::Miss => write!(f, "MISS"),
            CacheStatus::Stale => write!(f, "STALE"),
            CacheStatus::Bypass => write!(f, "BYPASS"),
        }
    }
}

/// Check if an ISR cache entry is fresh.
pub fn is_cache_fresh(entry: &IsrCacheEntry) -> bool {
    if let Some(revalidate_secs) = entry.revalidate_after_secs {
        let revalidate_duration = chrono::Duration::seconds(revalidate_secs);
        let stale_at = entry.last_revalidated_at + revalidate_duration;
        Utc::now() < stale_at
    } else {
        true
    }
}

/// Determine the route decision for a prerender route.
pub async fn resolve_prerender<S: SitesStore>(
    store: &Arc<S>,
    site_id: &SiteId,
    path: &str,
    decision: &RouteDecision,
) -> Result<(RouteDecision, CacheStatus), SitesError> {
    if let RouteDecision::Prerender {
        storage_key,
        revalidate_after: _,
        fallback,
    } = decision
    {
        if let Some(entry) = store.get_isr_entry(site_id, path).await? {
            if is_cache_fresh(&entry) {
                return Ok((
                    RouteDecision::StaticFile {
                        storage_key: entry.body_storage_key,
                        cache: Default::default(),
                        content_type: entry.content_type,
                    },
                    CacheStatus::Hit,
                ));
            } else {
                return Ok((
                    RouteDecision::StaticFile {
                        storage_key: entry.body_storage_key,
                        cache: Default::default(),
                        content_type: entry.content_type,
                    },
                    CacheStatus::Stale,
                ));
            }
        }

        if let Some(fallback) = fallback {
            return Ok((*fallback.clone(), CacheStatus::Miss));
        }

        return Ok((
            RouteDecision::StaticFile {
                storage_key: storage_key.clone(),
                cache: Default::default(),
                content_type: Some("text/html".to_string()),
            },
            CacheStatus::Miss,
        ));
    }

    Ok((decision.clone(), CacheStatus::Bypass))
}
