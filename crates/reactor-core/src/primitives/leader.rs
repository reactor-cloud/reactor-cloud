//! Leader election primitive for background task coordination.
//!
//! In clustered deployments, only one instance should run certain background
//! tasks (e.g., job scheduler, cleanup janitors). This trait provides
//! distributed locking semantics.
//!
//! # Implementations
//!
//! - [`AlwaysLeader`] — No-op implementation for single-node (always leader)
//! - `PgAdvisory` — PostgreSQL advisory locks (in `reactor-cache`)
//! - Redis lease — Redis-based distributed lock (future)

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Error type for leader election operations.
#[derive(Debug, Error)]
pub enum LeaderError {
    /// Failed to acquire lock.
    #[error("failed to acquire leadership: {0}")]
    AcquisitionFailed(String),
    /// Failed to release lock.
    #[error("failed to release leadership: {0}")]
    ReleaseFailed(String),
    /// Lock was lost unexpectedly.
    #[error("leadership lost")]
    LostLeadership,
    /// Connection error to the backend.
    #[error("connection error: {0}")]
    Connection(String),
}

/// Guard returned when leadership is acquired.
///
/// Leadership is held while this guard exists. Dropping the guard
/// releases leadership (best-effort in async context).
pub struct LeaderGuard {
    task_name: String,
    release_fn: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl LeaderGuard {
    /// Create a new leader guard.
    pub fn new(task_name: impl Into<String>) -> Self {
        Self {
            task_name: task_name.into(),
            release_fn: None,
        }
    }

    /// Create a leader guard with a release callback.
    pub fn with_release<F>(task_name: impl Into<String>, release: F) -> Self
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        Self {
            task_name: task_name.into(),
            release_fn: Some(Box::new(release)),
        }
    }

    /// Get the task name this guard is for.
    pub fn task_name(&self) -> &str {
        &self.task_name
    }
}

impl Drop for LeaderGuard {
    fn drop(&mut self) {
        if let Some(release) = self.release_fn.take() {
            release();
        }
    }
}

/// Leader election trait for background task coordination.
///
/// # Usage
///
/// ```ignore
/// let leader = get_leader_elect();
///
/// // Try to become leader for a task
/// if let Ok(guard) = leader.try_acquire("job-scheduler").await {
///     // We're the leader — run the scheduler
///     run_scheduler().await;
///     // Leadership released when guard is dropped
/// } else {
///     // Another instance is the leader — do nothing
/// }
/// ```
#[async_trait]
pub trait LeaderElect: Send + Sync {
    /// Try to acquire leadership for a named task.
    ///
    /// Returns `Ok(LeaderGuard)` if leadership was acquired, or an error
    /// if another instance is the leader or acquisition failed.
    ///
    /// # Arguments
    /// * `task_name` — Unique name for the task (e.g., "job-scheduler")
    async fn try_acquire(&self, task_name: &str) -> Result<LeaderGuard, LeaderError>;

    /// Acquire leadership, blocking until available.
    ///
    /// Will wait indefinitely until leadership is acquired. Use `try_acquire`
    /// for non-blocking attempts.
    async fn acquire(&self, task_name: &str) -> Result<LeaderGuard, LeaderError>;

    /// Acquire leadership with a timeout.
    ///
    /// Returns `Err` if leadership could not be acquired within the timeout.
    async fn acquire_with_timeout(
        &self,
        task_name: &str,
        timeout: Duration,
    ) -> Result<LeaderGuard, LeaderError>;

    /// Check if this instance is currently the leader for a task.
    ///
    /// This is a point-in-time check — leadership can be lost at any moment.
    async fn is_leader(&self, task_name: &str) -> bool;

    /// Release leadership for a task.
    ///
    /// Called automatically when `LeaderGuard` is dropped, but can be
    /// called explicitly for early release.
    async fn release(&self, task_name: &str) -> Result<(), LeaderError>;
}

/// No-op leader election that always grants leadership.
///
/// Use this in single-node deployments (G1/G2) where there's no need
/// for distributed coordination.
#[derive(Debug, Clone, Default)]
pub struct AlwaysLeader;

impl AlwaysLeader {
    /// Create a new always-leader implementation.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LeaderElect for AlwaysLeader {
    async fn try_acquire(&self, task_name: &str) -> Result<LeaderGuard, LeaderError> {
        Ok(LeaderGuard::new(task_name))
    }

    async fn acquire(&self, task_name: &str) -> Result<LeaderGuard, LeaderError> {
        Ok(LeaderGuard::new(task_name))
    }

    async fn acquire_with_timeout(
        &self,
        task_name: &str,
        _timeout: Duration,
    ) -> Result<LeaderGuard, LeaderError> {
        Ok(LeaderGuard::new(task_name))
    }

    async fn is_leader(&self, _task_name: &str) -> bool {
        true
    }

    async fn release(&self, _task_name: &str) -> Result<(), LeaderError> {
        Ok(())
    }
}

/// Create a shared always-leader instance.
#[must_use]
pub fn always_leader() -> Arc<dyn LeaderElect> {
    Arc::new(AlwaysLeader::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_always_leader_try_acquire() {
        let leader = AlwaysLeader::new();
        let guard = leader.try_acquire("test-task").await.unwrap();
        assert_eq!(guard.task_name(), "test-task");
    }

    #[tokio::test]
    async fn test_always_leader_is_leader() {
        let leader = AlwaysLeader::new();
        assert!(leader.is_leader("any-task").await);
    }

    #[tokio::test]
    async fn test_always_leader_acquire_multiple() {
        let leader = AlwaysLeader::new();

        // Should be able to acquire multiple times (no real locking)
        let _g1 = leader.try_acquire("task-1").await.unwrap();
        let _g2 = leader.try_acquire("task-1").await.unwrap();
        let _g3 = leader.try_acquire("task-2").await.unwrap();
    }

    #[tokio::test]
    async fn test_always_leader_release() {
        let leader = AlwaysLeader::new();
        leader.release("test-task").await.unwrap();
    }

    #[tokio::test]
    async fn test_leader_guard_drop() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let released = Arc::new(AtomicBool::new(false));
        let released_clone = released.clone();

        {
            let _guard =
                LeaderGuard::with_release("test", move || released_clone.store(true, Ordering::SeqCst));
        }

        assert!(released.load(Ordering::SeqCst));
    }
}
