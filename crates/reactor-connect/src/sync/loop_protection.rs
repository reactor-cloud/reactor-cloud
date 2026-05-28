//! Loop protection for bidirectional sync.
//!
//! Prevents sync loops by tracking recently synced records in reactor-cache KV.

use crate::error::ConnectError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Loop protection marker stored in KV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopMarker {
    /// Pair ID.
    pub pair_id: Uuid,
    /// Stream name.
    pub stream_name: String,
    /// Record primary key (serialized).
    pub record_key: String,
    /// Origin connection ID (the one that wrote this).
    pub origin_connection_id: Uuid,
    /// When the record was synced.
    pub synced_at: DateTime<Utc>,
    /// TTL expiry.
    pub expires_at: DateTime<Utc>,
}

/// Loop protection configuration.
#[derive(Debug, Clone)]
pub struct LoopProtectionConfig {
    /// Whether loop protection is enabled.
    pub enabled: bool,
    /// Window during which a record write is suppressed if it came from the other direction.
    pub window: Duration,
}

impl Default for LoopProtectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window: Duration::minutes(5),
        }
    }
}

/// Loop protection service.
pub struct LoopProtection {
    config: LoopProtectionConfig,
    // TODO: Inject reactor-cache client
}

impl LoopProtection {
    /// Create a new loop protection service.
    pub fn new(config: LoopProtectionConfig) -> Self {
        Self { config }
    }

    /// Generate the KV key for a loop marker.
    fn marker_key(pair_id: Uuid, stream: &str, record_key: &str) -> String {
        format!("connect:loop:{}:{}:{}", pair_id, stream, record_key)
    }

    /// Check if a record should be skipped due to loop protection.
    ///
    /// Returns `true` if the record was recently synced from the opposite direction
    /// and should be skipped to prevent a loop.
    pub async fn should_skip(
        &self,
        pair_id: Uuid,
        stream: &str,
        record_key: &str,
        current_connection_id: Uuid,
    ) -> Result<bool, ConnectError> {
        if !self.config.enabled {
            return Ok(false);
        }

        let key = Self::marker_key(pair_id, stream, record_key);

        // TODO: Lookup in reactor-cache KV
        // let marker: Option<LoopMarker> = cache.get(&key).await?;
        //
        // if let Some(marker) = marker {
        //     // Skip if the record was synced from a different connection within the window
        //     if marker.origin_connection_id != current_connection_id {
        //         let now = Utc::now();
        //         if marker.expires_at > now {
        //             return Ok(true);
        //         }
        //     }
        // }

        Ok(false)
    }

    /// Mark a record as synced (for loop protection).
    pub async fn mark_synced(
        &self,
        pair_id: Uuid,
        stream: &str,
        record_key: &str,
        connection_id: Uuid,
    ) -> Result<(), ConnectError> {
        if !self.config.enabled {
            return Ok(());
        }

        let key = Self::marker_key(pair_id, stream, record_key);
        let now = Utc::now();
        let expires_at = now + self.config.window;

        let marker = LoopMarker {
            pair_id,
            stream_name: stream.to_string(),
            record_key: record_key.to_string(),
            origin_connection_id: connection_id,
            synced_at: now,
            expires_at,
        };

        // TODO: Store in reactor-cache KV with TTL
        // cache.set_with_ttl(&key, &marker, self.config.window.num_seconds() as u64).await?;

        Ok(())
    }

    /// Clear loop markers for a pair (e.g., when disabling loop protection).
    pub async fn clear_pair(&self, pair_id: Uuid) -> Result<(), ConnectError> {
        // TODO: Scan and delete all keys matching connect:loop:{pair_id}:*
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marker_key() {
        let pair_id = Uuid::new_v4();
        let key = LoopProtection::marker_key(pair_id, "Lead", "abc123");
        assert!(key.starts_with("connect:loop:"));
        assert!(key.contains("Lead"));
        assert!(key.contains("abc123"));
    }

    #[tokio::test]
    async fn test_disabled_loop_protection() {
        let config = LoopProtectionConfig {
            enabled: false,
            window: Duration::minutes(5),
        };
        let lp = LoopProtection::new(config);

        let result = lp
            .should_skip(Uuid::new_v4(), "Lead", "key1", Uuid::new_v4())
            .await
            .unwrap();
        assert!(!result);
    }
}
