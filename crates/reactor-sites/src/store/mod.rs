//! Sites store abstractions.
//!
//! Provides traits and implementations for:
//! - SitesStore: Site, deployment, domain, and policy metadata in PostgreSQL

mod postgres;

pub use postgres::PgSitesStore;

use crate::error::SitesError;
use crate::Framework;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Site ID type alias.
pub type SiteId = Uuid;

/// Site deployment ID type alias.
pub type SiteDeploymentId = Uuid;

/// Route ID type alias.
pub type RouteId = Uuid;

/// Domain ID type alias.
pub type DomainId = Uuid;

/// Audit event ID type alias.
pub type AuditEventId = Uuid;

/// Site record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Site {
    /// Unique site ID.
    pub id: SiteId,
    /// Organization that owns this site.
    pub org_id: Uuid,
    /// Site name (unique within org).
    pub name: String,
    /// Framework type.
    pub framework: String,
    /// Currently promoted deployment (null until first promote).
    pub current_deployment_id: Option<SiteDeploymentId>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl Site {
    /// Parse the framework field into a Framework enum.
    pub fn framework(&self) -> Result<Framework, SitesError> {
        self.framework.parse()
    }
}

/// Input for creating a site.
#[derive(Debug, Clone)]
pub struct NewSite {
    /// Organization ID.
    pub org_id: Uuid,
    /// Site name.
    pub name: String,
    /// Framework type.
    pub framework: Framework,
}

/// Deployment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatus {
    /// Bundle uploaded, awaiting function materialization.
    Pending,
    /// Ready to receive traffic.
    Ready,
    /// Deployment failed.
    Failed,
    /// Resources cleaned up.
    Destroyed,
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentStatus::Pending => write!(f, "pending"),
            DeploymentStatus::Ready => write!(f, "ready"),
            DeploymentStatus::Failed => write!(f, "failed"),
            DeploymentStatus::Destroyed => write!(f, "destroyed"),
        }
    }
}

impl std::str::FromStr for DeploymentStatus {
    type Err = SitesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(DeploymentStatus::Pending),
            "ready" => Ok(DeploymentStatus::Ready),
            "failed" => Ok(DeploymentStatus::Failed),
            "destroyed" => Ok(DeploymentStatus::Destroyed),
            _ => Err(SitesError::Internal(format!(
                "invalid deployment status: {}",
                s
            ))),
        }
    }
}

/// Site deployment record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SiteDeployment {
    /// Unique deployment ID.
    pub id: SiteDeploymentId,
    /// Site this deployment belongs to.
    pub site_id: SiteId,
    /// Monotonically increasing version number.
    pub version: i64,
    /// Full manifest JSON.
    pub manifest_json: serde_json::Value,
    /// Current status.
    pub status: String,
    /// Error message if failed.
    pub status_detail: Option<String>,
    /// Number of static assets uploaded.
    pub static_asset_count: i32,
    /// Total bytes of static assets.
    pub static_asset_bytes: i64,
    /// When the deployment was created.
    pub deployed_at: DateTime<Utc>,
    /// User who created the deployment.
    pub deployed_by_user_id: Option<Uuid>,
}

/// Combined Site + SiteDeployment for host resolution queries.
#[derive(Debug, Clone, FromRow)]
pub struct SiteWithDeployment {
    // Site fields
    pub site_id: SiteId,
    pub org_id: Uuid,
    pub site_name: String,
    pub framework: String,
    pub current_deployment_id: Option<SiteDeploymentId>,
    pub site_created_at: DateTime<Utc>,
    pub site_updated_at: DateTime<Utc>,
    // Deployment fields
    pub deployment_id: SiteDeploymentId,
    pub deployment_site_id: SiteId,
    pub version: i64,
    pub manifest_json: serde_json::Value,
    pub status: String,
    pub status_detail: Option<String>,
    pub static_asset_count: i32,
    pub static_asset_bytes: i64,
    pub deployed_at: DateTime<Utc>,
    pub deployed_by_user_id: Option<Uuid>,
}

impl SiteWithDeployment {
    /// Split into separate Site and SiteDeployment structs.
    pub fn into_parts(self) -> (Site, SiteDeployment) {
        let site = Site {
            id: self.site_id,
            org_id: self.org_id,
            name: self.site_name,
            framework: self.framework,
            current_deployment_id: self.current_deployment_id,
            created_at: self.site_created_at,
            updated_at: self.site_updated_at,
        };
        let deployment = SiteDeployment {
            id: self.deployment_id,
            site_id: self.deployment_site_id,
            version: self.version,
            manifest_json: self.manifest_json,
            status: self.status,
            status_detail: self.status_detail,
            static_asset_count: self.static_asset_count,
            static_asset_bytes: self.static_asset_bytes,
            deployed_at: self.deployed_at,
            deployed_by_user_id: self.deployed_by_user_id,
        };
        (site, deployment)
    }
}

impl SiteDeployment {
    /// Parse the status field into a DeploymentStatus enum.
    pub fn status(&self) -> Result<DeploymentStatus, SitesError> {
        self.status.parse()
    }
}

/// Input for creating a deployment.
#[derive(Debug, Clone)]
pub struct NewDeployment {
    /// Site ID.
    pub site_id: SiteId,
    /// Manifest JSON.
    pub manifest_json: serde_json::Value,
    /// Deploying user ID.
    pub deployed_by_user_id: Option<Uuid>,
}

/// Deployment route record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeploymentRoute {
    /// Unique route ID.
    pub id: RouteId,
    /// Deployment this route belongs to.
    pub deployment_id: SiteDeploymentId,
    /// Path pattern (e.g., "/api/:path*").
    pub pattern: String,
    /// Method filter (null = any method).
    pub method_filter: Option<String>,
    /// Route kind: 'static', 'function', 'redirect', 'prerender'.
    pub route_kind: String,
    /// Target reference (storage key, function_id, URL, etc.).
    pub target_ref: String,
    /// Cache rules JSON.
    pub cache_rules_json: serde_json::Value,
    /// Priority (higher = matched first).
    pub priority: i32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Deployment function back-reference.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeploymentFunction {
    /// Deployment ID.
    pub deployment_id: SiteDeploymentId,
    /// Function ID (in reactor-functions).
    pub function_id: Uuid,
    /// Function role: 'ssr', 'api', 'isr-revalidate', etc.
    pub role: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Domain status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DomainStatus {
    /// Awaiting verification.
    Pending,
    /// DNS/HTTP verified, awaiting TLS.
    Verified,
    /// TLS certificate issued, ready to serve.
    Active,
    /// Verification or TLS provisioning failed.
    Failed,
}

impl std::fmt::Display for DomainStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainStatus::Pending => write!(f, "pending"),
            DomainStatus::Verified => write!(f, "verified"),
            DomainStatus::Active => write!(f, "active"),
            DomainStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for DomainStatus {
    type Err = SitesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(DomainStatus::Pending),
            "verified" => Ok(DomainStatus::Verified),
            "active" => Ok(DomainStatus::Active),
            "failed" => Ok(DomainStatus::Failed),
            _ => Err(SitesError::Internal(format!(
                "invalid domain status: {}",
                s
            ))),
        }
    }
}

/// Custom domain record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Domain {
    /// Unique domain ID.
    pub id: DomainId,
    /// Site this domain belongs to.
    pub site_id: SiteId,
    /// Domain host (e.g., "app.example.com").
    pub host: String,
    /// Current status.
    pub status: String,
    /// Verification token.
    pub verification_token: String,
    /// Verification method: 'dns' or 'http'.
    pub verification_method: String,
    /// TLS certificate reference.
    pub tls_cert_ref: Option<String>,
    /// TLS certificate expiration.
    pub tls_expires_at: Option<DateTime<Utc>>,
    /// When verification succeeded.
    pub verified_at: Option<DateTime<Utc>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Input for creating a domain.
#[derive(Debug, Clone)]
pub struct NewDomain {
    /// Site ID.
    pub site_id: SiteId,
    /// Domain host.
    pub host: String,
    /// Verification method.
    pub verification_method: String,
}

/// Per-site policy record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SitePolicy {
    /// Unique policy ID.
    pub id: Uuid,
    /// Site this policy belongs to.
    pub site_id: SiteId,
    /// Policy name.
    pub name: String,
    /// Compiled policy expression as JSON.
    pub using_expr_json: Option<serde_json::Value>,
    /// Original policy text.
    pub raw_text: String,
    /// SHA256 hash of the policy text.
    pub sha256: Vec<u8>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Input for creating a policy.
#[derive(Debug, Clone)]
pub struct NewSitePolicy {
    /// Site ID.
    pub site_id: SiteId,
    /// Policy name.
    pub name: String,
    /// Compiled policy expression.
    pub using_expr_json: Option<serde_json::Value>,
    /// Raw policy text.
    pub raw_text: String,
    /// SHA256 hash.
    pub sha256: Vec<u8>,
}

/// ISR cache entry.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IsrCacheEntry {
    /// Site ID.
    pub site_id: SiteId,
    /// Path being cached.
    pub path: String,
    /// Deployment that rendered this entry.
    pub deployment_id: SiteDeploymentId,
    /// Storage key for the cached body.
    pub body_storage_key: String,
    /// Content type.
    pub content_type: Option<String>,
    /// ETag for conditional requests.
    pub etag: Option<String>,
    /// Tags for invalidation (stored as JSON array).
    pub tags: serde_json::Value,
    /// Revalidate interval in seconds.
    pub revalidate_after_secs: Option<i64>,
    /// Last revalidation timestamp.
    pub last_revalidated_at: DateTime<Utc>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl IsrCacheEntry {
    /// Get the revalidate interval as a Duration.
    pub fn revalidate_after(&self) -> Option<std::time::Duration> {
        self.revalidate_after_secs.map(|s| std::time::Duration::from_secs(s as u64))
    }

    /// Get tags as a Vec.
    pub fn tags_vec(&self) -> Vec<String> {
        self.tags
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }
}

/// Audit event record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEvent {
    /// Unique event ID.
    pub id: AuditEventId,
    /// Event timestamp.
    pub ts: DateTime<Utc>,
    /// Actor user ID.
    pub actor_user_id: Option<Uuid>,
    /// Actor API key ID.
    pub actor_apikey_id: Option<Uuid>,
    /// Organization ID.
    pub org_id: Option<Uuid>,
    /// Site ID (if applicable).
    pub site_id: Option<SiteId>,
    /// Deployment ID (if applicable).
    pub deployment_id: Option<SiteDeploymentId>,
    /// Domain ID (if applicable).
    pub domain_id: Option<DomainId>,
    /// Event type.
    pub event_type: String,
    /// Additional event details.
    pub details: serde_json::Value,
    /// Request ID for tracing.
    pub request_id: String,
}

/// Input for creating an audit event.
#[derive(Debug, Clone)]
pub struct AuditEventCreate {
    /// Actor user ID.
    pub actor_user_id: Option<Uuid>,
    /// Actor API key ID.
    pub actor_apikey_id: Option<Uuid>,
    /// Organization ID.
    pub org_id: Option<Uuid>,
    /// Site ID.
    pub site_id: Option<SiteId>,
    /// Deployment ID.
    pub deployment_id: Option<SiteDeploymentId>,
    /// Domain ID.
    pub domain_id: Option<DomainId>,
    /// Event type.
    pub event_type: String,
    /// Event details.
    pub details: serde_json::Value,
    /// Request ID.
    pub request_id: String,
}

/// Sites store trait.
#[async_trait]
pub trait SitesStore: Send + Sync {
    // Site CRUD
    /// Create a new site.
    async fn create_site(&self, site: &NewSite) -> Result<Site, SitesError>;

    /// Get a site by org and name.
    async fn get_site(&self, org_id: &Uuid, name: &str) -> Result<Option<Site>, SitesError>;

    /// Get a site by ID.
    async fn get_site_by_id(&self, id: &SiteId) -> Result<Option<Site>, SitesError>;

    /// List sites for an org.
    async fn list_sites(&self, org_id: &Uuid) -> Result<Vec<Site>, SitesError>;

    /// Delete a site.
    async fn delete_site(&self, id: &SiteId) -> Result<(), SitesError>;

    // Deployments
    /// Create a new deployment.
    async fn create_deployment(&self, d: &NewDeployment) -> Result<SiteDeployment, SitesError>;

    /// Get a deployment by ID.
    async fn get_deployment(&self, id: &SiteDeploymentId)
        -> Result<Option<SiteDeployment>, SitesError>;

    /// Get the current deployment for a site.
    async fn current_deployment(&self, site_id: &SiteId)
        -> Result<Option<SiteDeployment>, SitesError>;

    /// Promote a deployment (set as current).
    async fn promote_deployment(&self, id: &SiteDeploymentId) -> Result<(), SitesError>;

    /// List deployments for a site.
    async fn list_deployments(
        &self,
        site_id: &SiteId,
        limit: u32,
    ) -> Result<Vec<SiteDeployment>, SitesError>;

    /// Update deployment status.
    async fn update_deployment_status(
        &self,
        id: &SiteDeploymentId,
        status: DeploymentStatus,
        detail: Option<&str>,
    ) -> Result<(), SitesError>;

    /// Update deployment asset stats.
    async fn update_deployment_assets(
        &self,
        id: &SiteDeploymentId,
        count: i32,
        bytes: i64,
    ) -> Result<(), SitesError>;

    /// Get the next deployment version for a site.
    async fn next_deployment_version(&self, site_id: &SiteId) -> Result<i64, SitesError>;

    // Deployment routes
    /// Set routes for a deployment (replaces existing).
    async fn set_deployment_routes(
        &self,
        deployment_id: &SiteDeploymentId,
        routes: &[DeploymentRoute],
    ) -> Result<(), SitesError>;

    /// Get routes for a deployment.
    async fn get_deployment_routes(
        &self,
        deployment_id: &SiteDeploymentId,
    ) -> Result<Vec<DeploymentRoute>, SitesError>;

    // Deployment functions
    /// Add a function reference to a deployment.
    async fn add_deployment_function(
        &self,
        deployment_id: &SiteDeploymentId,
        function_id: &Uuid,
        role: &str,
    ) -> Result<(), SitesError>;

    /// Get all function references for a deployment.
    async fn get_deployment_functions(
        &self,
        deployment_id: &SiteDeploymentId,
    ) -> Result<Vec<DeploymentFunction>, SitesError>;

    // Custom domains
    /// Create a new domain.
    async fn create_domain(&self, d: &NewDomain) -> Result<Domain, SitesError>;

    /// Get a domain by host.
    async fn get_domain(&self, host: &str) -> Result<Option<Domain>, SitesError>;

    /// List domains for a site.
    async fn list_domains(&self, site_id: &SiteId) -> Result<Vec<Domain>, SitesError>;

    /// Update domain status.
    async fn update_domain_status(
        &self,
        id: &DomainId,
        status: DomainStatus,
        cert_ref: Option<&str>,
    ) -> Result<(), SitesError>;

    /// Delete a domain.
    async fn delete_domain(&self, id: &DomainId) -> Result<(), SitesError>;

    // Host resolution (for serve plane)
    /// Look up site + deployment by host header.
    async fn get_site_by_host(
        &self,
        host: &str,
    ) -> Result<Option<(Site, SiteDeployment)>, SitesError>;

    // ISR cache
    /// Get an ISR cache entry.
    async fn get_isr_entry(
        &self,
        site_id: &SiteId,
        path: &str,
    ) -> Result<Option<IsrCacheEntry>, SitesError>;

    /// Set an ISR cache entry.
    async fn set_isr_entry(&self, entry: &IsrCacheEntry) -> Result<(), SitesError>;

    /// Invalidate ISR entries by path or tag.
    async fn invalidate_isr(&self, site_id: &SiteId, path_or_tag: &str) -> Result<u32, SitesError>;

    // Policies
    /// Get all policies for a site.
    async fn get_site_policies(&self, site_id: &SiteId) -> Result<Vec<SitePolicy>, SitesError>;

    /// Upsert a policy.
    async fn upsert_policy(&self, p: &NewSitePolicy) -> Result<SitePolicy, SitesError>;

    /// Delete a policy.
    async fn delete_policy(&self, id: &Uuid) -> Result<(), SitesError>;

    // Audit
    /// Write an audit event.
    async fn write_audit_event(&self, event: &AuditEventCreate) -> Result<(), SitesError>;
}
