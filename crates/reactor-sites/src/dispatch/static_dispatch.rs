//! Static file dispatch via reactor-storage.

use crate::error::SitesError;
use reqwest::Client;
use std::sync::Arc;

/// HTTP client for reactor-storage service.
pub struct StorageClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl StorageClient {
    /// Create a new storage client.
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

    /// Get a signed URL for an object.
    pub async fn get_signed_url(
        &self,
        bucket: &str,
        key: &str,
        expires_secs: u64,
    ) -> Result<String, SitesError> {
        // Note: storage routes are /storage/v1/sign/:bucket/:key
        let url = format!(
            "{}/storage/v1/sign/{}/{}?expires={}",
            self.base_url, bucket, key, expires_secs
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| SitesError::Storage(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(SitesError::Storage(format!("status {}: {}", status, text)));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| SitesError::Storage(e.to_string()))?;

        body["signed_url"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SitesError::Storage("missing signed_url in response".to_string()))
    }

    /// Upload an object.
    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        content_type: &str,
        body: bytes::Bytes,
    ) -> Result<(), SitesError> {
        // Note: storage routes are /storage/v1/object/:bucket/:key (singular "object")
        let url = format!(
            "{}/storage/v1/object/{}/{}",
            self.base_url, bucket, key
        );

        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", content_type)
            .body(body)
            .send()
            .await
            .map_err(|e| SitesError::StaticUploadFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(SitesError::StaticUploadFailed(format!(
                "status {}: {}",
                status, text
            )));
        }

        Ok(())
    }

    /// Get an object's content.
    pub async fn get_object(&self, bucket: &str, key: &str) -> Result<bytes::Bytes, SitesError> {
        let url = format!(
            "{}/storage/v1/object/{}/{}",
            self.base_url, bucket, key
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| SitesError::Storage(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(SitesError::Storage(format!("status {}: {}", status, text)));
        }

        response
            .bytes()
            .await
            .map_err(|e| SitesError::Storage(e.to_string()))
    }

    /// Delete an object.
    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), SitesError> {
        let url = format!(
            "{}/storage/v1/object/{}/{}",
            self.base_url, bucket, key
        );

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| SitesError::Storage(e.to_string()))?;

        if !response.status().is_success() && response.status() != reqwest::StatusCode::NOT_FOUND {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(SitesError::Storage(format!("status {}: {}", status, text)));
        }

        Ok(())
    }

    /// Check if the storage service is reachable.
    pub async fn health_check(&self) -> Result<(), SitesError> {
        let url = format!("{}/storage/v1/health", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SitesError::Storage(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(SitesError::Storage(format!(
                "health check failed: {}",
                response.status()
            )))
        }
    }

    /// Ensure the system bucket exists.
    pub async fn ensure_system_bucket(&self) -> Result<(), SitesError> {
        let url = format!("{}/storage/v1/buckets", self.base_url);

        let body = serde_json::json!({
            "name": "_reactor_sites",
            "public": false,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| SitesError::Storage(e.to_string()))?;

        if response.status().is_success() || response.status() == reqwest::StatusCode::CONFLICT {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(SitesError::Storage(format!(
                "failed to create system bucket: {} {}",
                status, text
            )))
        }
    }
}

/// Static file dispatcher.
pub struct StaticDispatcher {
    storage: Arc<StorageClient>,
}

impl StaticDispatcher {
    /// Create a new static dispatcher.
    pub fn new(storage: Arc<StorageClient>) -> Self {
        Self { storage }
    }

    /// Get the storage client.
    pub fn storage(&self) -> &Arc<StorageClient> {
        &self.storage
    }
}

// Re-export StorageClient from dispatch module
pub use self::StorageClient as StorageClientType;
