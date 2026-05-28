//! Functions capability client (`/fn/v1/*`).

use crate::error::ClientResult;
use crate::http::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Function metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub id: Uuid,
    pub name: String,
    pub runtime: String,
    pub status: String,
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
    #[serde(default = "default_timeout_sec")]
    pub timeout_sec: u32,
    pub current_deployment_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn default_memory_mb() -> u32 {
    256
}

fn default_timeout_sec() -> u32 {
    30
}

/// Function deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub id: Uuid,
    pub function_id: Uuid,
    pub version: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Environment variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

/// Log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: HashMap<String, serde_json::Value>,
}

/// Invocation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationResult {
    pub invocation_id: Uuid,
    pub status: String,
    pub duration_ms: u64,
    #[serde(default)]
    pub response: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

impl Client {
    /// List functions.
    pub async fn functions_list(&self) -> ClientResult<Vec<Function>> {
        self.get("/fn/v1/_admin/functions").await
    }

    /// Get function details.
    pub async fn functions_get(&self, name: &str) -> ClientResult<Function> {
        self.get(&format!("/fn/v1/_admin/functions/{}", name)).await
    }

    /// Deploy a function.
    pub async fn functions_deploy(&self, name: &str, bundle: Vec<u8>, runtime: &str) -> ClientResult<Deployment> {
        use reqwest::multipart::{Form, Part};

        let part = Part::bytes(bundle)
            .file_name("function.zip")
            .mime_str("application/zip")?;
        let form = Form::new()
            .part("bundle", part)
            .text("name", name.to_string())
            .text("runtime", runtime.to_string());

        self.post_multipart("/fn/v1/_admin/deployments", form).await
    }

    /// Rollback to a specific deployment.
    pub async fn functions_rollback(&self, name: &str, deployment_id: Uuid) -> ClientResult<Function> {
        #[derive(Serialize)]
        struct Rollback {
            deployment_id: Uuid,
        }
        self.post(
            &format!("/fn/v1/_admin/functions/{}/rollback", name),
            &Rollback { deployment_id },
        )
        .await
    }

    /// Invoke a function.
    pub async fn functions_invoke(
        &self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> ClientResult<InvocationResult> {
        self.post(
            &format!("/fn/v1/{}", name),
            &data.unwrap_or(serde_json::Value::Null),
        )
        .await
    }

    /// List environment variables for a function.
    pub async fn functions_env_list(&self, name: &str) -> ClientResult<Vec<EnvVar>> {
        self.get(&format!("/fn/v1/_admin/functions/{}/env", name))
            .await
    }

    /// Get an environment variable.
    pub async fn functions_env_get(&self, name: &str, key: &str) -> ClientResult<EnvVar> {
        self.get(&format!("/fn/v1/_admin/functions/{}/env/{}", name, key))
            .await
    }

    /// Set an environment variable.
    pub async fn functions_env_set(&self, name: &str, key: &str, value: &str) -> ClientResult<EnvVar> {
        #[derive(Serialize)]
        struct SetEnv<'a> {
            value: &'a str,
        }
        self.put(
            &format!("/fn/v1/_admin/functions/{}/env/{}", name, key),
            &SetEnv { value },
        )
        .await
    }

    /// Unset an environment variable.
    pub async fn functions_env_unset(&self, name: &str, key: &str) -> ClientResult<()> {
        self.delete::<serde_json::Value>(&format!(
            "/fn/v1/_admin/functions/{}/env/{}",
            name, key
        ))
        .await?;
        Ok(())
    }

    /// Get function logs.
    pub async fn functions_logs(
        &self,
        name: &str,
        since: Option<&str>,
        limit: Option<u32>,
    ) -> ClientResult<Vec<LogEntry>> {
        let mut path = format!("/fn/v1/_admin/functions/{}/logs", name);
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

    /// List deployments for a function.
    pub async fn functions_deployments_list(&self, name: &str) -> ClientResult<Vec<Deployment>> {
        self.get(&format!("/fn/v1/_admin/functions/{}/deployments", name))
            .await
    }
}
