//! Routing model for the edge gateway.
//!
//! Defines the data structures for routing requests to backend services.

use chrono::{DateTime, Utc};
use reactor_core::{ProjectId, ProjectRef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TLS mode for a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
    /// Use the wildcard certificate (*.reactor.cloud).
    Wildcard,
    /// Use on-demand TLS (for custom domains).
    OnDemand,
    /// Manual certificate management.
    Manual,
}

impl Default for TlsMode {
    fn default() -> Self {
        Self::Wildcard
    }
}

impl std::fmt::Display for TlsMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wildcard => write!(f, "wildcard"),
            Self::OnDemand => write!(f, "on_demand"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

impl std::str::FromStr for TlsMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "wildcard" => Ok(Self::Wildcard),
            "on_demand" => Ok(Self::OnDemand),
            "manual" => Ok(Self::Manual),
            _ => Err(format!("unknown TLS mode: {}", s)),
        }
    }
}

/// Backend deployment kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// Dedicated instance for this project.
    Dedicated,
    /// Shared pool of instances.
    Shared,
}

impl Default for BackendKind {
    fn default() -> Self {
        Self::Dedicated
    }
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dedicated => write!(f, "dedicated"),
            Self::Shared => write!(f, "shared"),
        }
    }
}

impl std::str::FromStr for BackendKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dedicated" => Ok(Self::Dedicated),
            "shared" => Ok(Self::Shared),
            _ => Err(format!("unknown backend kind: {}", s)),
        }
    }
}

/// Backend target address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendTarget {
    /// Internal address (e.g., "reactor-cloud.internal:8000").
    pub address: String,
    /// Health check path.
    #[serde(default = "default_health_path")]
    pub health_path: String,
}

fn default_health_path() -> String {
    "/_admin/health".to_string()
}

impl BackendTarget {
    /// Create a new backend target.
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            health_path: default_health_path(),
        }
    }

    /// Set the health check path.
    pub fn with_health_path(mut self, path: impl Into<String>) -> Self {
        self.health_path = path.into();
        self
    }
}

/// A single route in the routing table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Hostname for this route.
    pub host: String,
    /// Project ID this route belongs to.
    pub project_id: ProjectId,
    /// Project reference (URL-safe identifier).
    pub project_ref: ProjectRef,
    /// Backend deployment kind.
    pub backend_kind: BackendKind,
    /// Backend target address.
    pub backend_target: BackendTarget,
    /// TLS mode for this route.
    pub tls_mode: TlsMode,
    /// Whether this route is enabled.
    pub enabled: bool,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl Route {
    /// Create a new route.
    pub fn new(
        host: impl Into<String>,
        project_id: ProjectId,
        project_ref: ProjectRef,
        backend_target: BackendTarget,
    ) -> Self {
        Self {
            host: host.into(),
            project_id,
            project_ref,
            backend_kind: BackendKind::default(),
            backend_target,
            tls_mode: TlsMode::default(),
            enabled: true,
            updated_at: Utc::now(),
        }
    }

    /// Set the backend kind.
    pub fn with_backend_kind(mut self, kind: BackendKind) -> Self {
        self.backend_kind = kind;
        self
    }

    /// Set the TLS mode.
    pub fn with_tls_mode(mut self, mode: TlsMode) -> Self {
        self.tls_mode = mode;
        self
    }

    /// Disable this route.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Custom domain entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomDomain {
    /// Domain hostname.
    pub host: String,
    /// Project ID this domain belongs to.
    pub project_id: ProjectId,
    /// DNS verification token.
    pub verification_token: String,
    /// When the domain was verified (None if not yet verified).
    pub verified_at: Option<DateTime<Utc>>,
    /// Certificate provisioning status.
    pub cert_status: CertStatus,
    /// When this entry was created.
    pub created_at: DateTime<Utc>,
}

/// Certificate provisioning status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertStatus {
    /// Certificate pending provisioning.
    Pending,
    /// Certificate provisioning in progress.
    Provisioning,
    /// Certificate active.
    Active,
    /// Certificate provisioning failed.
    Failed,
}

impl Default for CertStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for CertStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Provisioning => write!(f, "provisioning"),
            Self::Active => write!(f, "active"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for CertStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "provisioning" => Ok(Self::Provisioning),
            "active" => Ok(Self::Active),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown cert status: {}", s)),
        }
    }
}

impl CustomDomain {
    /// Create a new custom domain entry.
    pub fn new(host: impl Into<String>, project_id: ProjectId, verification_token: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            project_id,
            verification_token: verification_token.into(),
            verified_at: None,
            cert_status: CertStatus::default(),
            created_at: Utc::now(),
        }
    }

    /// Check if this domain is verified.
    pub fn is_verified(&self) -> bool {
        self.verified_at.is_some()
    }

    /// Mark this domain as verified.
    pub fn mark_verified(mut self) -> Self {
        self.verified_at = Some(Utc::now());
        self
    }
}

/// In-memory routing table.
#[derive(Debug, Clone, Default)]
pub struct RoutingTable {
    routes: HashMap<String, Route>,
}

impl RoutingTable {
    /// Create a new empty routing table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a routing table from a list of routes.
    pub fn from_routes(routes: Vec<Route>) -> Self {
        let routes = routes.into_iter().map(|r| (r.host.clone(), r)).collect();
        Self { routes }
    }

    /// Get a route by host.
    pub fn get(&self, host: &str) -> Option<&Route> {
        self.routes.get(host)
    }

    /// Add or update a route.
    pub fn upsert(&mut self, route: Route) {
        self.routes.insert(route.host.clone(), route);
    }

    /// Remove a route.
    pub fn remove(&mut self, host: &str) -> Option<Route> {
        self.routes.remove(host)
    }

    /// Get all routes.
    pub fn all(&self) -> impl Iterator<Item = &Route> {
        self.routes.values()
    }

    /// Get all enabled routes.
    pub fn enabled(&self) -> impl Iterator<Item = &Route> {
        self.routes.values().filter(|r| r.enabled)
    }

    /// Number of routes.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// Get routes matching a project.
    pub fn for_project<'a>(&'a self, project_id: &'a ProjectId) -> impl Iterator<Item = &'a Route> {
        self.routes.values().filter(move |r| &r.project_id == project_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_mode_roundtrip() {
        for mode in [TlsMode::Wildcard, TlsMode::OnDemand, TlsMode::Manual] {
            let s = mode.to_string();
            let parsed: TlsMode = s.parse().unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_backend_kind_roundtrip() {
        for kind in [BackendKind::Dedicated, BackendKind::Shared] {
            let s = kind.to_string();
            let parsed: BackendKind = s.parse().unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn test_routing_table_operations() {
        let mut table = RoutingTable::new();
        assert!(table.is_empty());

        let project_id = ProjectId::new();
        let project_ref = ProjectRef::from(&project_id);
        let target = BackendTarget::new("backend:8000");

        let route = Route::new("example.reactor.cloud", project_id.clone(), project_ref, target);
        table.upsert(route.clone());

        assert_eq!(table.len(), 1);
        assert!(table.get("example.reactor.cloud").is_some());
        assert!(table.get("nonexistent.reactor.cloud").is_none());

        let found: Vec<_> = table.for_project(&project_id).collect();
        assert_eq!(found.len(), 1);
    }
}
