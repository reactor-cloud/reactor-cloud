//! Run execution.

use uuid::Uuid;

use crate::error::JobsError;
use crate::state::JobsState;
use crate::store::{JobsStore, PgJobsStore, RunStatus};

/// Execute a job run.
pub async fn execute_run(state: &JobsState, run_id: Uuid) -> Result<(), JobsError> {
    let store = PgJobsStore::new(state.pool.clone());

    // Load the run
    let run = store
        .get_run(run_id)
        .await?
        .ok_or_else(|| JobsError::RunNotFound(run_id.to_string()))?;

    // Check status
    if run.status != "pending" && run.status != "sleeping" {
        tracing::debug!(run_id = %run_id, status = %run.status, "skipping run with unexpected status");
        return Ok(());
    }

    // Load the job
    let job = store
        .get_job_by_id(run.job_id)
        .await?
        .ok_or_else(|| JobsError::JobNotFound(run.job_id.to_string()))?;

    // Update status to running
    store
        .update_run_status(run_id, RunStatus::Running, None, None)
        .await?;

    // Load completed steps for replay
    let steps = store.list_steps(run_id).await?;
    let step_cache: std::collections::HashMap<String, serde_json::Value> = steps
        .into_iter()
        .filter(|s| s.status == "completed")
        .filter_map(|s| s.output_json.map(|output| (s.name, output)))
        .collect();

    // Load state
    let state_entries = store.list_state(run_id).await?;
    let run_state: std::collections::HashMap<String, serde_json::Value> = state_entries
        .into_iter()
        .map(|e| (e.key, e.value_json))
        .collect();

    // Build job context header
    let context = serde_json::json!({
        "run_id": run_id.to_string(),
        "job_name": job.name,
        "attempt": run.attempt,
        "step_cache": step_cache,
        "state": run_state,
    });

    // Invoke the function via reactor-functions
    let function_url = format!("{}/fn/v1/{}", state.config.functions_url, job.function_name);

    let response = state
        .http_client
        .post(&function_url)
        .header("Authorization", format!("Bearer {}", state.config.functions_api_key))
        .header("X-Reactor-Job-Context", context.to_string())
        .header("Content-Type", "application/json")
        .body(run.payload_json.to_string())
        .timeout(std::time::Duration::from_millis(job.timeout_ms as u64))
        .send()
        .await
        .map_err(|e| JobsError::Internal(format!("function invoke failed: {}", e)))?;

    if response.status().is_success() {
        // Update status to succeeded
        store
            .update_run_status(run_id, RunStatus::Succeeded, None, None)
            .await?;

        tracing::info!(run_id = %run_id, job = %job.name, "run succeeded");
    } else {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();

        // Check if we should retry
        if run.attempt < run.max_attempts {
            // Increment attempt and set back to pending
            store.increment_run_attempt(run_id).await?;
            tracing::warn!(
                run_id = %run_id,
                job = %job.name,
                attempt = run.attempt,
                max_attempts = run.max_attempts,
                status = %status,
                "run failed, will retry"
            );
        } else {
            // Max attempts reached, move to DLQ
            store
                .update_run_status(
                    run_id,
                    RunStatus::Failed,
                    Some("max_attempts_exceeded"),
                    Some(&error_body),
                )
                .await?;
            store.move_to_dlq(run_id, &error_body).await?;

            tracing::error!(
                run_id = %run_id,
                job = %job.name,
                attempt = run.attempt,
                "run failed, moved to DLQ"
            );
        }
    }

    Ok(())
}
