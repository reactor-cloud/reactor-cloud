//! Job scheduler.
//!
//! The scheduler handles:
//! - Cron trigger polling
//! - Event matching
//! - Sleep wakeups

pub mod cron;
pub mod event;
pub mod sleep;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

use crate::state::JobsState;

/// Start the scheduler loops.
pub async fn start_scheduler(
    state: JobsState,
    mut shutdown: watch::Receiver<bool>,
    interval: Duration,
) {
    let state = Arc::new(state);

    // Spawn cron polling task
    let cron_state = state.clone();
    let mut cron_shutdown = shutdown.clone();
    let cron_interval = interval;
    let cron_handle = tokio::spawn(async move {
        cron::run_cron_loop(cron_state, &mut cron_shutdown, cron_interval).await;
    });

    // Spawn event matching task
    let event_state = state.clone();
    let mut event_shutdown = shutdown.clone();
    let event_interval = interval;
    let event_handle = tokio::spawn(async move {
        event::run_event_loop(event_state, &mut event_shutdown, event_interval).await;
    });

    // Spawn sleep wakeup task
    let sleep_state = state.clone();
    let mut sleep_shutdown = shutdown.clone();
    let sleep_interval = interval;
    let sleep_handle = tokio::spawn(async move {
        sleep::run_sleep_loop(sleep_state, &mut sleep_shutdown, sleep_interval).await;
    });

    // Wait for shutdown signal
    let _ = shutdown.changed().await;

    // Wait for all tasks to complete
    let _ = tokio::join!(cron_handle, event_handle, sleep_handle);
}
