//! Function dispatch via reactor-functions HTTP API.

use crate::error::SitesError;
use reqwest::Client;
use std::sync::Arc;

/// HTTP client for reactor-functions service.
pub struct FunctionsClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl FunctionsClient {
    /// Create a new functions client.
    pub fn new(base_url: String, api_key: String) -> Arc<Self> {
        Arc::new(Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        })
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Invoke a function by name.
    pub async fn invoke(
        &self,
        function_name: &str,
        sub_path: &str,
        method: reqwest::Method,
        headers: reqwest::header::HeaderMap,
        body: bytes::Bytes,
    ) -> Result<reqwest::Response, SitesError> {
        let url = format!("{}/fn/v1/{}/{}", self.base_url, function_name, sub_path);

        let response = self
            .client
            .request(method, &url)
            .headers(headers)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .body(body)
            .send()
            .await
            .map_err(|e| SitesError::FunctionDispatchFailed(e.to_string()))?;

        Ok(response)
    }

    /// Create a function (for deploying synthetic internal functions).
    pub async fn create_function(
        &self,
        org_id: &str,
        name: &str,
        runtime: &str,
        description: Option<&str>,
    ) -> Result<serde_json::Value, SitesError> {
        let url = format!("{}/fn/v1/_admin/functions", self.base_url);

        let body = serde_json::json!({
            "name": name,
            "runtime": runtime,
            "description": description,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-Reactor-Org", org_id)
            .json(&body)
            .send()
            .await
            .map_err(|e| SitesError::FunctionDeployFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(SitesError::FunctionDeployFailed(format!(
                "status {}: {}",
                status, text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| SitesError::FunctionDeployFailed(e.to_string()))
    }

    /// Delete a function.
    pub async fn delete_function(&self, org_id: &str, name: &str) -> Result<(), SitesError> {
        let url = format!("{}/fn/v1/_admin/functions/{}", self.base_url, name);

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-Reactor-Org", org_id)
            .send()
            .await
            .map_err(|e| SitesError::Functions(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(SitesError::Functions(format!("status {}: {}", status, text)));
        }

        Ok(())
    }

    /// Check if the functions service is reachable.
    pub async fn health_check(&self) -> Result<(), SitesError> {
        let url = format!("{}/fn/v1/health", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SitesError::Functions(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(SitesError::Functions(format!(
                "health check failed: {}",
                response.status()
            )))
        }
    }
}
