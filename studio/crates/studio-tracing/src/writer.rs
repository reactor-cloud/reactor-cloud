use std::path::{Path, PathBuf};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::types::TraceStep;

#[derive(Debug, thiserror::Error)]
pub enum WriterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub struct TraceWriter {
    log_path: PathBuf,
}

impl TraceWriter {
    pub fn new(workspace_path: &Path) -> Self {
        Self {
            log_path: workspace_path.join(".reactor/logs/agent.jsonl"),
        }
    }

    pub async fn ensure_dir(&self) -> Result<(), WriterError> {
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        Ok(())
    }

    pub async fn write_step(&self, conversation_id: &str, step: &TraceStep) -> Result<(), WriterError> {
        self.ensure_dir().await?;

        let entry = serde_json::json!({
            "conversationId": conversation_id,
            "step": step,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }
}

pub struct AppLogWriter {
    log_path: PathBuf,
}

impl AppLogWriter {
    pub fn new(workspace_path: &Path) -> Self {
        Self {
            log_path: workspace_path.join(".reactor/logs/app.jsonl"),
        }
    }

    pub async fn ensure_dir(&self) -> Result<(), WriterError> {
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        Ok(())
    }

    pub async fn write_event(
        &self,
        level: &str,
        category: &str,
        event: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), WriterError> {
        self.ensure_dir().await?;

        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": level,
            "category": category,
            "event": event,
            "data": data,
        });

        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }
}
