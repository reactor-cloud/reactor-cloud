//! Worker pool management.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

use crate::state::JobsState;
use crate::worker::executor;

/// Worker pool for processing job runs.
pub struct WorkerPool {
    worker_count: usize,
}

impl WorkerPool {
    /// Create a new worker pool.
    pub fn new(worker_count: usize) -> Self {
        Self { worker_count }
    }

    /// Start the worker pool.
    pub async fn start(
        &self,
        state: JobsState,
        shutdown: watch::Receiver<bool>,
        visibility_timeout: Duration,
    ) {
        let state = Arc::new(state);
        let mut handles = Vec::with_capacity(self.worker_count);

        for worker_id in 0..self.worker_count {
            let worker_state = state.clone();
            let mut worker_shutdown = shutdown.clone();
            let timeout = visibility_timeout;

            let handle = tokio::spawn(async move {
                run_worker(worker_id, worker_state, &mut worker_shutdown, timeout).await;
            });

            handles.push(handle);
        }

        // Wait for all workers to complete
        for handle in handles {
            let _ = handle.await;
        }
    }
}

async fn run_worker(
    worker_id: usize,
    state: Arc<JobsState>,
    shutdown: &mut watch::Receiver<bool>,
    visibility_timeout: Duration,
) {
    tracing::info!(worker_id, "worker starting");

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                tracing::info!(worker_id, "worker shutting down");
                break;
            }
            _ = process_next_run(&state, visibility_timeout) => {
                // Continue processing
            }
        }
    }
}

async fn process_next_run(state: &JobsState, visibility_timeout: Duration) {
    // TODO: Implement proper queue selection based on orgs
    // For now, try to dequeue from a default queue
    use reactor_cache::QueueOperations;

    // Try to dequeue a run
    let items = match state
        .cache
        .dequeue("jobs:default", 1, visibility_timeout)
        .await
    {
        Ok(items) => items,
        Err(e) => {
            tracing::debug!(error = %e, "no items to dequeue");
            tokio::time::sleep(Duration::from_millis(100)).await;
            return;
        }
    };

    if items.is_empty() {
        tokio::time::sleep(Duration::from_millis(100)).await;
        return;
    }

    let item = &items[0];

    // Parse run ID from data
    let run_id = match uuid::Uuid::from_slice(&item.data) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = %e, "failed to parse run ID from queue item");
            // Ack to remove bad item
            let _ = state.cache.ack("jobs:default", &item.receipt).await;
            return;
        }
    };

    // Execute the run
    match executor::execute_run(state, run_id).await {
        Ok(_) => {
            // Ack the item
            let _ = state.cache.ack("jobs:default", &item.receipt).await;
        }
        Err(e) => {
            tracing::error!(run_id = %run_id, error = %e, "run execution failed");
            // Nack with delay for retry
            let _ = state
                .cache
                .nack("jobs:default", &item.receipt, Some(Duration::from_secs(5)))
                .await;
        }
    }
}
