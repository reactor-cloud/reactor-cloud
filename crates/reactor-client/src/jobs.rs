//! Jobs capability client (`/jobs/v1/*`).

use crate::error::ClientResult;
use crate::http::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Job definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub name: String,
    pub function_name: String,
    pub trigger: JobTrigger,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Job trigger configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobTrigger {
    Cron { schedule: String },
    Event { event_type: String },
    Manual,
}

/// Job run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    pub job_id: Uuid,
    pub status: RunStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Dead letter queue entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqEntry {
    pub id: Uuid,
    pub job_id: Uuid,
    pub run_id: Uuid,
    pub error: String,
    pub payload: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub attempts: u32,
}

/// Log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobLogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: HashMap<String, serde_json::Value>,
}

impl Client {
    /// List jobs.
    pub async fn jobs_list(&self) -> ClientResult<Vec<Job>> {
        self.get("/jobs/v1/_admin/jobs").await
    }

    /// Get job details.
    pub async fn jobs_get(&self, name: &str) -> ClientResult<Job> {
        self.get(&format!("/jobs/v1/_admin/jobs/{}", name)).await
    }

    /// Manually trigger a job.
    pub async fn jobs_trigger(
        &self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> ClientResult<Run> {
        self.post(
            &format!("/jobs/v1/_admin/jobs/{}/trigger", name),
            &data.unwrap_or(serde_json::Value::Null),
        )
        .await
    }

    /// List runs for a job.
    pub async fn jobs_runs_list(&self, name: &str, limit: Option<u32>) -> ClientResult<Vec<Run>> {
        let mut path = format!("/jobs/v1/_admin/jobs/{}/runs", name);
        if let Some(l) = limit {
            path.push_str(&format!("?limit={}", l));
        }
        self.get(&path).await
    }

    /// Get run details.
    pub async fn jobs_run_get(&self, run_id: Uuid) -> ClientResult<Run> {
        self.get(&format!("/jobs/v1/_admin/runs/{}", run_id)).await
    }

    /// List DLQ entries.
    pub async fn jobs_dlq_list(&self, job_name: Option<&str>) -> ClientResult<Vec<DlqEntry>> {
        let path = match job_name {
            Some(name) => format!("/jobs/v1/_admin/dlq?job={}", name),
            None => "/jobs/v1/_admin/dlq".to_string(),
        };
        self.get(&path).await
    }

    /// Replay a DLQ entry.
    pub async fn jobs_dlq_replay(&self, dlq_id: Uuid) -> ClientResult<Run> {
        self.post(&format!("/jobs/v1/_admin/dlq/{}/replay", dlq_id), &())
            .await
    }

    /// Purge DLQ entries.
    pub async fn jobs_dlq_purge(&self, job_name: Option<&str>) -> ClientResult<u32> {
        #[derive(Deserialize)]
        struct PurgeResult {
            purged: u32,
        }
        let path = match job_name {
            Some(name) => format!("/jobs/v1/_admin/dlq/purge?job={}", name),
            None => "/jobs/v1/_admin/dlq/purge".to_string(),
        };
        let result: PurgeResult = self.post(&path, &()).await?;
        Ok(result.purged)
    }

    /// Get job logs.
    pub async fn jobs_logs(
        &self,
        name: &str,
        since: Option<&str>,
        limit: Option<u32>,
    ) -> ClientResult<Vec<JobLogEntry>> {
        let mut path = format!("/jobs/v1/_admin/jobs/{}/logs", name);
        let mut params = vec![];
        if let Some(s) = since {
            params.push(format!("since={}", s));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if !params.is_empty() {
            path.push('?');
            path.push_str(&params.join("&"));
        }
        self.get(&path).await
    }
}
