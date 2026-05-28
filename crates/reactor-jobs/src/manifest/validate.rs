//! Manifest validation.

use crate::error::JobsError;
use crate::manifest::{JobManifest, TriggerConfig};
use crate::EVENT_TOPIC_REGEX;

/// Validate a job manifest.
pub fn validate_manifest(manifest: &JobManifest) -> Result<(), JobsError> {
    // Validate triggers
    for trigger in &manifest.triggers {
        validate_trigger(trigger)?;
    }

    // Validate retry config
    if manifest.retry.max_attempts == 0 {
        return Err(JobsError::InvalidTriggerConfig(
            "max_attempts must be at least 1".to_string(),
        ));
    }

    if manifest.retry.initial_delay_ms == 0 {
        return Err(JobsError::InvalidTriggerConfig(
            "initial_delay_ms must be greater than 0".to_string(),
        ));
    }

    // Validate concurrency
    if manifest.max_concurrency == 0 {
        return Err(JobsError::InvalidTriggerConfig(
            "max_concurrency must be at least 1".to_string(),
        ));
    }

    // Validate timeout
    if manifest.timeout_ms == 0 {
        return Err(JobsError::InvalidTriggerConfig(
            "timeout_ms must be greater than 0".to_string(),
        ));
    }

    Ok(())
}

fn validate_trigger(trigger: &TriggerConfig) -> Result<(), JobsError> {
    match trigger {
        TriggerConfig::Cron { schedule } => {
            // Validate cron expression
            cron::Schedule::from_str(schedule)
                .map_err(|e| JobsError::InvalidCron(format!("{}: {}", schedule, e)))?;
        }
        TriggerConfig::Event { topic } => {
            // Validate topic format
            if !EVENT_TOPIC_REGEX.is_match(topic) {
                return Err(JobsError::InvalidEventTopic(topic.clone()));
            }
        }
        TriggerConfig::Webhook {} | TriggerConfig::Manual {} => {
            // No validation needed
        }
    }

    Ok(())
}

use std::str::FromStr;
