//! Sleep wakeup handling.

use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

use crate::state::JobsState;
use crate::store::{JobsStore, PgJobsStore, RunStatus};

/// Run the sleep wakeup loop.
pub async fn run_sleep_loop(
    state: Arc<JobsState>,
    shutdown: &mut watch::Receiver<bool>,
    interval: Duration,
) {
    tracing::info!("starting sleep wakeup loop");

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                tracing::info!("sleep wakeup loop shutting down");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                if let Err(e) = poll_sleeping_runs(&state).await {
                    tracing::error!(error = %e, "sleep wakeup polling error");
                }
            }
        }
    }
}

async fn poll_sleeping_runs(state: &JobsState) -> Result<(), crate::error::JobsError> {
    let store = PgJobsStore::new(state.pool.clone());
    let now = Utc::now();

    // Get sleeping runs that are due to wake up
    let runs = store.list_sleeping_runs_due(now).await?;

    for run in runs {
        // Update status to pending
        store
            .update_run_status(run.id, RunStatus::Pending, None, None)
            .await?;

        // Re-enqueue the run
        let queue_name = format!("jobs:{}", run.org_id);
        state
            .cache
            .enqueue(&queue_name, run.id.as_bytes(), None)
            .await?;

        tracing::info!(
            run_id = %run.id,
            "sleeping run woken up"
        );
    }

    Ok(())
}

use reactor_cache::QueueOperations;
