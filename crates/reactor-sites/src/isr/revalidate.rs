//! ISR revalidation management.

use crate::error::SitesError;
use crate::store::SiteId;

/// Revalidation manager for ISR.
pub struct RevalidationManager {
    jobs_url: Option<String>,
    jobs_api_key: Option<String>,
}

impl RevalidationManager {
    /// Create a new revalidation manager.
    pub fn new(jobs_url: Option<String>, jobs_api_key: Option<String>) -> Self {
        Self {
            jobs_url,
            jobs_api_key,
        }
    }

    /// Enqueue a background revalidation job.
    pub async fn enqueue_revalidation(
        &self,
        site_id: &SiteId,
        path: &str,
        function_name: &str,
    ) -> Result<(), SitesError> {
        let (jobs_url, jobs_api_key) = match (&self.jobs_url, &self.jobs_api_key) {
            (Some(url), Some(key)) => (url, key),
            _ => {
                tracing::warn!(
                    "jobs service not configured, skipping background revalidation for {}",
                    path
                );
                return Ok(());
            }
        };

        let client = reqwest::Client::new();
        let url = format!("{}/jobs/v1/_admin/jobs", jobs_url);

        let body = serde_json::json!({
            "name": format!("isr-revalidate-{}-{}", site_id, path.replace('/', "-")),
            "function_name": function_name,
            "payload": {
                "site_id": site_id.to_string(),
                "path": path,
            },
        });

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", jobs_api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| SitesError::RevalidateFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(SitesError::RevalidateFailed(format!(
                "failed to enqueue job: {} {}",
                status, text
            )));
        }

        Ok(())
    }
}
