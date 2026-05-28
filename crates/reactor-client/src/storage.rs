//! Storage capability client (`/storage/v1/*`).

use crate::error::ClientResult;
use crate::http::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Storage bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bucket {
    pub id: Uuid,
    pub name: String,
    pub public: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Storage policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePolicy {
    pub id: Uuid,
    pub bucket_id: Uuid,
    pub name: String,
    pub definition: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Client {
    /// List buckets.
    pub async fn storage_buckets_list(&self) -> ClientResult<Vec<Bucket>> {
        self.get("/storage/v1/_admin/buckets").await
    }

    /// Get bucket details.
    pub async fn storage_bucket_get(&self, name: &str) -> ClientResult<Bucket> {
        self.get(&format!("/storage/v1/_admin/buckets/{}", name))
            .await
    }

    /// Create a bucket.
    pub async fn storage_bucket_create(&self, name: &str, public: bool) -> ClientResult<Bucket> {
        #[derive(Serialize)]
        struct CreateBucket<'a> {
            name: &'a str,
            public: bool,
        }
        self.post("/storage/v1/_admin/buckets", &CreateBucket { name, public })
            .await
    }

    /// Delete a bucket.
    pub async fn storage_bucket_delete(&self, name: &str) -> ClientResult<()> {
        self.delete::<serde_json::Value>(&format!("/storage/v1/_admin/buckets/{}", name))
            .await?;
        Ok(())
    }

    /// List policies for a bucket.
    pub async fn storage_policies_list(&self, bucket: &str) -> ClientResult<Vec<StoragePolicy>> {
        self.get(&format!("/storage/v1/_admin/buckets/{}/policies", bucket))
            .await
    }

    /// Create a policy.
    pub async fn storage_policy_create(
        &self,
        bucket: &str,
        name: &str,
        definition: &str,
    ) -> ClientResult<StoragePolicy> {
        #[derive(Serialize)]
        struct CreatePolicy<'a> {
            name: &'a str,
            definition: &'a str,
        }
        self.post(
            &format!("/storage/v1/_admin/buckets/{}/policies", bucket),
            &CreatePolicy { name, definition },
        )
        .await
    }

    /// Delete a policy.
    pub async fn storage_policy_delete(&self, bucket: &str, policy_id: Uuid) -> ClientResult<()> {
        self.delete::<serde_json::Value>(&format!(
            "/storage/v1/_admin/buckets/{}/policies/{}",
            bucket, policy_id
        ))
        .await?;
        Ok(())
    }
}
