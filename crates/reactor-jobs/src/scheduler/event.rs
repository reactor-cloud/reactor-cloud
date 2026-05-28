//! Event trigger matching.

use reactor_cache::QueueOperations;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

use crate::manifest::TriggerKind;
use crate::state::JobsState;
use crate::store::{JobsStore, NewRun, PgJobsStore};

/// Run the event matching loop.
pub async fn run_event_loop(
    state: Arc<JobsState>,
    shutdown: &mut watch::Receiver<bool>,
    interval: Duration,
) {
    tracing::info!("starting event scheduler loop");

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                tracing::info!("event scheduler shutting down");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                if let Err(e) = poll_events(&state).await {
                    tracing::error!(error = %e, "event polling error");
                }
            }
        }
    }
}

async fn poll_events(state: &JobsState) -> Result<(), crate::error::JobsError> {
    let store = PgJobsStore::new(state.pool.clone());

    // Get all orgs with pending events (simplified: get recent events)
    // In a real implementation, we'd track which orgs have pending events
    let events = store.list_pending_events_for_org(uuid::Uuid::nil(), 100).await?;

    for event in events {
        // Find triggers that match this event's topic
        // Query triggers where kind='event' AND config_json->>'topic' = event.topic
        // This is a simplified implementation - in production we'd have a more efficient index
        
        let matching_triggers: Vec<_> = sqlx::query_as::<_, TriggerRow>(
            r#"
            SELECT t.* FROM _reactor_jobs.triggers t
            JOIN _reactor_jobs.jobs j ON t.job_id = j.id
            WHERE t.kind = 'event'
              AND t.enabled = true
              AND j.org_id = $1
              AND t.config_json->>'topic' = $2
            "#,
        )
        .bind(event.org_id)
        .bind(&event.topic)
        .fetch_all(&state.pool)
        .await
        .map_err(crate::error::JobsError::Database)?;

        for trigger in matching_triggers {
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
                    topic = %event.topic,
                    "skipping event trigger due to concurrency limit"
                );
                continue;
            }

            // Create a run for this trigger
            let new_run = NewRun {
                job_id: job.id,
                org_id: job.org_id,
                trigger_id: Some(trigger.id),
                trigger_kind: TriggerKind::Event,
                payload_json: event.payload_json.clone(),
                max_attempts: job.retry_max_attempts,
            };

            let run = store.create_run(&new_run).await?;

            // Enqueue the run
            let queue_name = format!("jobs:{}", job.org_id);
            state
                .cache
                .enqueue(&queue_name, run.id.as_bytes(), None)
                .await?;

            // Mark event as consumed
            store.consume_event(event.id, run.id).await?;

            tracing::info!(
                job = %job.name,
                run_id = %run.id,
                topic = %event.topic,
                "event trigger fired"
            );
        }
    }

    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct TriggerRow {
    id: uuid::Uuid,
    job_id: uuid::Uuid,
    #[allow(dead_code)]
    kind: String,
    #[allow(dead_code)]
    config_json: serde_json::Value,
    #[allow(dead_code)]
    enabled: bool,
}
