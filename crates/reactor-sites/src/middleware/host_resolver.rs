//! Host header resolver for serve plane.

use crate::error::SitesError;
use crate::store::{Site, SiteDeployment, SiteDeploymentId, SitesStore};
use std::sync::Arc;
use uuid::Uuid;

/// Resolved host information.
#[derive(Debug, Clone)]
pub struct ResolvedHost {
    /// The site being served.
    pub site: Site,
    /// The deployment being served.
    pub deployment: SiteDeployment,
    /// Whether this is a preview deployment.
    pub is_preview: bool,
}

/// Host resolver for the serve plane.
pub struct HostResolver<S: SitesStore> {
    store: Arc<S>,
    preview_subdomain: String,
}

impl<S: SitesStore> HostResolver<S> {
    /// Create a new host resolver.
    pub fn new(store: Arc<S>, preview_subdomain: String) -> Self {
        Self {
            store,
            preview_subdomain,
        }
    }

    /// Resolve a host header to a site and deployment.
    pub async fn resolve(&self, host: &str) -> Result<Option<ResolvedHost>, SitesError> {
        if let Some(preview_info) = self.parse_preview_host(host) {
            return self.resolve_preview(&preview_info.deployment_id, &preview_info.site_host).await;
        }

        if let Some((site, deployment)) = self.store.get_site_by_host(host).await? {
            return Ok(Some(ResolvedHost {
                site,
                deployment,
                is_preview: false,
            }));
        }

        if let Some(site_name) = self.parse_reactor_host(host) {
            return self.resolve_by_pattern(&site_name).await;
        }

        Ok(None)
    }

    /// Parse a preview subdomain pattern.
    fn parse_preview_host(&self, host: &str) -> Option<PreviewHostInfo> {
        let preview_pattern = format!(".{}.preview.", self.preview_subdomain);
        let parts: Vec<&str> = host.split('.').collect();

        if parts.len() >= 4 {
            let deployment_id_str = parts[0];
            if parts[1] == "preview" {
                if let Ok(deployment_id) = Uuid::parse_str(deployment_id_str) {
                    let site_host = parts[2..].join(".");
                    return Some(PreviewHostInfo {
                        deployment_id,
                        site_host,
                    });
                }
            }
        }

        None
    }

    /// Parse a *.reactor.app host pattern.
    fn parse_reactor_host(&self, host: &str) -> Option<String> {
        if host.ends_with(".reactor.app") {
            let without_suffix = host.strip_suffix(".reactor.app")?;
            let parts: Vec<&str> = without_suffix.split('.').collect();
            if !parts.is_empty() {
                return Some(parts[0].to_string());
            }
        }
        None
    }

    async fn resolve_preview(
        &self,
        deployment_id: &SiteDeploymentId,
        _site_host: &str,
    ) -> Result<Option<ResolvedHost>, SitesError> {
        if let Some(deployment) = self.store.get_deployment(deployment_id).await? {
            if let Some(site) = self.store.get_site_by_id(&deployment.site_id).await? {
                return Ok(Some(ResolvedHost {
                    site,
                    deployment,
                    is_preview: true,
                }));
            }
        }
        Ok(None)
    }

    async fn resolve_by_pattern(&self, _site_name: &str) -> Result<Option<ResolvedHost>, SitesError> {
        Ok(None)
    }
}

#[derive(Debug)]
struct PreviewHostInfo {
    deployment_id: SiteDeploymentId,
    site_host: String,
}
