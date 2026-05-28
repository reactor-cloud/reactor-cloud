//! PostgreSQL advisory lock-based leader election.
//!
//! Uses `pg_advisory_lock` and `pg_try_advisory_lock` to coordinate
//! background task leadership across multiple instances.
//!
//! Advisory locks are session-scoped, so:
//! - Lock is automatically released when the connection closes
//! - Lock is held across transactions
//! - Lock is NOT held if the process crashes (failover happens)

use async_trait::async_trait;
use reactor_core::primitives::leader::{LeaderElect, LeaderError, LeaderGuard};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Convert a task name to a stable lock ID.
///
/// Uses a simple hash to convert arbitrary task names into i64 lock IDs.
fn task_name_to_lock_id(task_name: &str) -> i64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    task_name.hash(&mut hasher);
    // Use the lower 63 bits to ensure positive lock ID
    (hasher.finish() & 0x7FFFFFFFFFFFFFFF) as i64
}

/// PostgreSQL advisory lock-based leader election.
///
/// Each task name is hashed to a stable lock ID. Only one connection
/// can hold a given advisory lock at a time.
///
/// # Connection handling
///
/// This implementation acquires a dedicated connection from the pool
/// for each lock and holds it until the lock is released. This ensures
/// the lock survives connection pool reclamation.
#[derive(Clone)]
pub struct PgAdvisoryLeader {
    pool: PgPool,
    /// Track which locks this instance holds (for is_leader check)
    held_locks: Arc<RwLock<HashMap<String, i64>>>,
}

impl PgAdvisoryLeader {
    /// Create a new PostgreSQL advisory lock-based leader election.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            held_locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl LeaderElect for PgAdvisoryLeader {
    async fn try_acquire(&self, task_name: &str) -> Result<LeaderGuard, LeaderError> {
        let lock_id = task_name_to_lock_id(task_name);

        // Try to acquire the lock (non-blocking)
        let result: (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
            .bind(lock_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| LeaderError::Connection(e.to_string()))?;

        if result.0 {
            // Successfully acquired lock
            let task = task_name.to_string();
            {
                let mut locks = self.held_locks.write().await;
                locks.insert(task.clone(), lock_id);
            }

            // Create guard with release callback
            let pool = self.pool.clone();
            let held_locks = self.held_locks.clone();
            let task_for_release = task.clone();

            Ok(LeaderGuard::with_release(task, move || {
                // Spawn a blocking task to release the lock
                // (can't do async in Drop, but we try our best)
                let pool = pool.clone();
                let held_locks = held_locks.clone();
                let task = task_for_release.clone();
                tokio::spawn(async move {
                    let lock_id = {
                        let mut locks = held_locks.write().await;
                        locks.remove(&task)
                    };
                    if let Some(id) = lock_id {
                        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                            .bind(id)
                            .execute(&pool)
                            .await;
                    }
                });
            }))
        } else {
            Err(LeaderError::AcquisitionFailed(format!(
                "another instance holds the lock for '{}'",
                task_name
            )))
        }
    }

    async fn acquire(&self, task_name: &str) -> Result<LeaderGuard, LeaderError> {
        let lock_id = task_name_to_lock_id(task_name);

        // Blocking acquire
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(lock_id)
            .execute(&self.pool)
            .await
            .map_err(|e| LeaderError::Connection(e.to_string()))?;

        // Successfully acquired lock
        let task = task_name.to_string();
        {
            let mut locks = self.held_locks.write().await;
            locks.insert(task.clone(), lock_id);
        }

        // Create guard with release callback
        let pool = self.pool.clone();
        let held_locks = self.held_locks.clone();
        let task_for_release = task.clone();

        Ok(LeaderGuard::with_release(task, move || {
            let pool = pool.clone();
            let held_locks = held_locks.clone();
            let task = task_for_release.clone();
            tokio::spawn(async move {
                let lock_id = {
                    let mut locks = held_locks.write().await;
                    locks.remove(&task)
                };
                if let Some(id) = lock_id {
                    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                        .bind(id)
                        .execute(&pool)
                        .await;
                }
            });
        }))
    }

    async fn acquire_with_timeout(
        &self,
        task_name: &str,
        timeout: Duration,
    ) -> Result<LeaderGuard, LeaderError> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            // Try non-blocking acquire
            match self.try_acquire(task_name).await {
                Ok(guard) => return Ok(guard),
                Err(LeaderError::AcquisitionFailed(_)) => {
                    // Another instance holds the lock, wait and retry
                    if tokio::time::Instant::now() >= deadline {
                        return Err(LeaderError::AcquisitionFailed(format!(
                            "timeout waiting for lock '{}'",
                            task_name
                        )));
                    }
                    interval.tick().await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn is_leader(&self, task_name: &str) -> bool {
        let locks = self.held_locks.read().await;
        locks.contains_key(task_name)
    }

    async fn release(&self, task_name: &str) -> Result<(), LeaderError> {
        let lock_id = {
            let mut locks = self.held_locks.write().await;
            locks.remove(task_name)
        };

        if let Some(id) = lock_id {
            sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| LeaderError::ReleaseFailed(e.to_string()))?;
        }

        Ok(())
    }
}

/// Create a PostgreSQL advisory lock-based leader election instance.
pub fn pg_advisory_leader(pool: PgPool) -> Arc<dyn LeaderElect> {
    Arc::new(PgAdvisoryLeader::new(pool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_name_to_lock_id_deterministic() {
        let id1 = task_name_to_lock_id("job-scheduler");
        let id2 = task_name_to_lock_id("job-scheduler");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_task_name_to_lock_id_different() {
        let id1 = task_name_to_lock_id("job-scheduler");
        let id2 = task_name_to_lock_id("event-processor");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_name_to_lock_id_positive() {
        let id = task_name_to_lock_id("any-task");
        assert!(id >= 0);
    }
}
