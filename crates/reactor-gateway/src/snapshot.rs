//! Configuration snapshot for atomic apply and rollback.
//!
//! This module provides a way to apply configuration changes atomically
//! with automatic rollback on failure.

use crate::caddy_admin::CaddyAdminClient;
use crate::error::{GatewayError, GatewayResult};
use crate::routing::RoutingTable;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// A snapshot of the Caddy configuration.
#[derive(Debug, Clone)]
pub struct ConfigSnapshot {
    /// The Caddy JSON configuration.
    pub config: Value,
    /// When this snapshot was created.
    pub created_at: DateTime<Utc>,
    /// Number of routes in this snapshot.
    pub route_count: usize,
}

impl ConfigSnapshot {
    /// Create a new snapshot from a configuration.
    pub fn new(config: Value, route_count: usize) -> Self {
        Self {
            config,
            created_at: Utc::now(),
            route_count,
        }
    }
}

/// Manager for configuration snapshots with rollback capability.
pub struct SnapshotManager {
    client: Arc<CaddyAdminClient>,
    last_good: RwLock<Option<ConfigSnapshot>>,
    max_rollback_attempts: usize,
}

impl SnapshotManager {
    /// Create a new snapshot manager.
    pub fn new(client: Arc<CaddyAdminClient>) -> Self {
        Self {
            client,
            last_good: RwLock::new(None),
            max_rollback_attempts: 3,
        }
    }

    /// Apply a new configuration with automatic rollback on failure.
    ///
    /// If the apply fails, attempts to rollback to the last known good configuration.
    pub async fn apply(&self, table: &RoutingTable) -> GatewayResult<()> {
        let new_config = self.client.build_config(table)?;
        let route_count = table.len();

        debug!(
            "Applying new configuration with {} routes",
            route_count
        );

        // Try to apply the new configuration
        match self.client.load_config(&new_config).await {
            Ok(()) => {
                info!(
                    "Successfully applied configuration with {} routes",
                    route_count
                );

                // Save as last known good
                let snapshot = ConfigSnapshot::new(new_config, route_count);
                *self.last_good.write().await = Some(snapshot);

                Ok(())
            }
            Err(e) => {
                error!("Failed to apply configuration: {}", e);

                // Attempt rollback
                self.rollback().await?;

                Err(e)
            }
        }
    }

    /// Attempt to rollback to the last known good configuration.
    pub async fn rollback(&self) -> GatewayResult<()> {
        let last_good = self.last_good.read().await.clone();

        match last_good {
            Some(snapshot) => {
                warn!(
                    "Rolling back to configuration from {} with {} routes",
                    snapshot.created_at, snapshot.route_count
                );

                for attempt in 1..=self.max_rollback_attempts {
                    match self.client.load_config(&snapshot.config).await {
                        Ok(()) => {
                            info!("Rollback successful on attempt {}", attempt);
                            return Ok(());
                        }
                        Err(e) => {
                            error!(
                                "Rollback attempt {} failed: {}",
                                attempt, e
                            );

                            if attempt == self.max_rollback_attempts {
                                return Err(GatewayError::RollbackFailed(format!(
                                    "All {} rollback attempts failed",
                                    self.max_rollback_attempts
                                )));
                            }

                            // Brief delay before retry
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }
                }

                Err(GatewayError::RollbackFailed(
                    "Exhausted rollback attempts".to_string(),
                ))
            }
            None => {
                warn!("No previous configuration to rollback to");
                Err(GatewayError::RollbackFailed(
                    "No previous configuration available".to_string(),
                ))
            }
        }
    }

    /// Get the last known good snapshot.
    pub async fn get_last_good(&self) -> Option<ConfigSnapshot> {
        self.last_good.read().await.clone()
    }

    /// Clear the last known good snapshot.
    pub async fn clear(&self) {
        *self.last_good.write().await = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation() {
        let config = serde_json::json!({});
        let snapshot = ConfigSnapshot::new(config.clone(), 10);

        assert_eq!(snapshot.route_count, 10);
        assert_eq!(snapshot.config, config);
    }
}
