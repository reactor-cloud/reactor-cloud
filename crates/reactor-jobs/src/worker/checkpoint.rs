//! Step checkpoint management.
//!
//! Handles persisting and retrieving step outputs for replay on retry.

use uuid::Uuid;

use crate::error::JobsError;
use crate::store::{JobsStore, NewStep, PgJobsStore, StepStatus};

/// Checkpoint manager for step execution.
pub struct CheckpointManager {
    store: PgJobsStore,
    run_id: Uuid,
}

impl CheckpointManager {
    /// Create a new checkpoint manager.
    pub fn new(store: PgJobsStore, run_id: Uuid) -> Self {
        Self { store, run_id }
    }

    /// Get cached output for a step if it exists and is completed.
    pub async fn get_cached_output(
        &self,
        step_name: &str,
    ) -> Result<Option<serde_json::Value>, JobsError> {
        let step = self.store.get_step(self.run_id, step_name).await?;

        match step {
            Some(s) if s.status == "completed" => Ok(s.output_json),
            _ => Ok(None),
        }
    }

    /// Start a step (creates row with status=running).
    pub async fn start_step(
        &self,
        step_name: &str,
        input: Option<&serde_json::Value>,
    ) -> Result<Uuid, JobsError> {
        let step = self
            .store
            .create_step(&NewStep {
                run_id: self.run_id,
                name: step_name.to_string(),
                input_json: input.cloned(),
            })
            .await?;

        Ok(step.id)
    }

    /// Complete a step (updates with output).
    pub async fn complete_step(
        &self,
        step_id: Uuid,
        output: &serde_json::Value,
    ) -> Result<(), JobsError> {
        self.store
            .update_step(step_id, StepStatus::Completed, Some(output), None)
            .await
    }

    /// Fail a step (updates with error).
    pub async fn fail_step(&self, step_id: Uuid, error: &str) -> Result<(), JobsError> {
        self.store
            .update_step(step_id, StepStatus::Failed, None, Some(error))
            .await
    }
}
