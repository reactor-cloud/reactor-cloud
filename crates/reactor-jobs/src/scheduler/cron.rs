//! Cron trigger polling.

use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

use crate::state::JobsState;
use crate::store::{JobsStore, NewRun, PgJobsStore};

/// Run the cron polling loop.
pub async fn run_cron_loop(
    state: Arc<JobsState>,
    shutdown: &mut watch::Receiver<bool>,
    interval: Duration,
) {
    tracing::info!("starting cron scheduler loop");

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                tracing::info!("cron scheduler shutting down");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                if let Err(e) = poll_cron_triggers(&state).await {
                    tracing::error!(error = %e, "cron polling error");
                }
            }
        }
    }
}

async fn poll_cron_triggers(state: &JobsState) -> Result<(), crate::error::JobsError> {
    let store = PgJobsStore::new(state.pool.clone());
    let now = Utc::now();

    // Get due cron triggers (with FOR UPDATE SKIP LOCKED)
    let triggers = store.list_due_cron_triggers(now).await?;

    for trigger in triggers {
        // Get the job
        let job = match store.get_job_by_id(trigger.job_id).await? {
            Some(j) => j,
            None => continue,
        };

        // Check concurrency
        let active_runs = store.count_active_runs(job.id).await?;
        if active_runs >= job.max_concurrency as u32 {
            tracing::debug!(
                job = %job.name,
                active_runs,
                max_concurrency = job.max_concurrency,
                "skipping cron trigger due to concurrency limit"
            );
            continue;
        }

        // Create a new run
        let new_run = NewRun {
            job_id: job.id,
            org_id: job.org_id,
            trigger_id: Some(trigger.id),
            trigger_kind: crate::manifest::TriggerKind::Cron,
            payload_json: serde_json::json!({}),
            max_attempts: job.retry_max_attempts,
        };

        let run = store.create_run(&new_run).await?;

        // Enqueue the run
        let queue_name = format!("jobs:{}", job.org_id);
        state
            .cache
            .enqueue(&queue_name, run.id.as_bytes(), None)
            .await?;

        // Update trigger timestamps
        let next_trigger_at = compute_next_trigger(&trigger.config_json);
        store
            .update_trigger_fired(trigger.id, now, next_trigger_at)
            .await?;

        tracing::info!(
            job = %job.name,
            run_id = %run.id,
            "cron trigger fired"
        );
    }

    Ok(())
}

fn compute_next_trigger(config: &serde_json::Value) -> Option<chrono::DateTime<Utc>> {
    let schedule_str = config.get("schedule")?.as_str()?;
    let schedule = cron::Schedule::from_str(schedule_str).ok()?;
    schedule.upcoming(Utc).next()
}

use reactor_cache::QueueOperations;
use std::str::FromStr;
