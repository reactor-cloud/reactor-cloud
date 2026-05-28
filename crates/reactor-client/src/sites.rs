//! Sites capability client (`/sites/v1/*`).

use crate::error::ClientResult;
use crate::http::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Site metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    pub id: Uuid,
    pub name: String,
    pub framework: String,
    pub current_deployment_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Site deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteDeployment {
    pub id: Uuid,
    pub site_id: Uuid,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Custom domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: Uuid,
    pub site_id: Uuid,
    pub domain: String,
    pub status: DomainStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Domain verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainStatus {
    Pending,
    Verified,
    Failed,
}

/// Log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteLogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: HashMap<String, serde_json::Value>,
}

impl Client {
    /// List sites.
    pub async fn sites_list(&self) -> ClientResult<Vec<Site>> {
        self.get("/sites/v1/_admin/sites").await
    }

    /// Get site details.
    pub async fn sites_get(&self, name: &str) -> ClientResult<Site> {
        self.get(&format!("/sites/v1/_admin/sites/{}", name)).await
    }

    /// Deploy a site.
    pub async fn sites_deploy(&self, name: &str, bundle: Vec<u8>, framework: &str) -> ClientResult<SiteDeployment> {
        use reqwest::multipart::{Form, Part};

        let part = Part::bytes(bundle)
            .file_name("site.tar.zst")
            .mime_str("application/zstd")?;
        let form = Form::new()
            .part("bundle", part)
            .text("name", name.to_string())
            .text("framework", framework.to_string());

        self.post_multipart("/sites/v1/_admin/deployments", form).await
    }

    /// Promote a deployment.
    pub async fn sites_promote(&self, name: &str, deployment_id: Uuid) -> ClientResult<Site> {
        #[derive(Serialize)]
        struct Promote {
            deployment_id: Uuid,
        }
        self.post(
            &format!("/sites/v1/_admin/sites/{}/promote", name),
            &Promote { deployment_id },
        )
        .await
    }

    /// Rollback to a previous deployment.
    pub async fn sites_rollback(&self, name: &str) -> ClientResult<Site> {
        self.post(&format!("/sites/v1/_admin/sites/{}/rollback", name), &())
            .await
    }

    /// List custom domains for a site.
    pub async fn sites_domains_list(&self, name: &str) -> ClientResult<Vec<Domain>> {
        self.get(&format!("/sites/v1/_admin/sites/{}/domains", name))
            .await
    }

    /// Add a custom domain.
    pub async fn sites_domain_add(&self, name: &str, domain: &str) -> ClientResult<Domain> {
        #[derive(Serialize)]
        struct AddDomain<'a> {
            domain: &'a str,
        }
        self.post(
            &format!("/sites/v1/_admin/sites/{}/domains", name),
            &AddDomain { domain },
        )
        .await
    }

    /// Remove a custom domain.
    pub async fn sites_domain_remove(&self, name: &str, domain_id: Uuid) -> ClientResult<()> {
        self.delete::<serde_json::Value>(&format!(
            "/sites/v1/_admin/sites/{}/domains/{}",
            name, domain_id
        ))
        .await?;
        Ok(())
    }

    /// Verify a custom domain.
    pub async fn sites_domain_verify(&self, name: &str, domain_id: Uuid) -> ClientResult<Domain> {
        self.post(
            &format!("/sites/v1/_admin/sites/{}/domains/{}/verify", name, domain_id),
            &(),
        )
        .await
    }

    /// Revalidate ISR cache.
    pub async fn sites_revalidate(&self, name: &str, path: &str) -> ClientResult<()> {
        #[derive(Serialize)]
        struct Revalidate<'a> {
            path: &'a str,
        }
        self.post::<serde_json::Value, _>(
            &format!("/sites/v1/_admin/sites/{}/revalidate", name),
            &Revalidate { path },
        )
        .await?;
        Ok(())
    }

    /// Get site logs.
    pub async fn sites_logs(
        &self,
        name: &str,
        since: Option<&str>,
        limit: Option<u32>,
    ) -> ClientResult<Vec<SiteLogEntry>> {
        let mut path = format!("/sites/v1/_admin/sites/{}/logs", name);
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

    /// List site deployments.
    pub async fn sites_deployments_list(&self, name: &str) -> ClientResult<Vec<SiteDeployment>> {
        self.get(&format!("/sites/v1/_admin/sites/{}/deployments", name))
            .await
    }
}
