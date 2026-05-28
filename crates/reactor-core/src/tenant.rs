//! Tenant context types for multi-tenancy support.
//!
//! [`TenantCtx`] provides per-request tenant information that capabilities
//! use to scope their operations. In single-tenant mode (G2), this is a
//! fixed value from config. In multi-tenant mode (G3c shared cluster),
//! it's resolved from the request's Host header or JWT claims.

use crate::project::{ProjectId, ProjectRef};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

/// Tenant environment — the deployment stage of a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TenantEnv {
    /// Production environment.
    #[default]
    Production,
    /// Preview/staging environment.
    Preview,
    /// Local development environment.
    Dev,
}

impl TenantEnv {
    /// Returns the environment as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Preview => "preview",
            Self::Dev => "dev",
        }
    }

    /// Check if this is a production environment.
    #[must_use]
    pub const fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    /// Check if this is a development environment.
    #[must_use]
    pub const fn is_dev(&self) -> bool {
        matches!(self, Self::Dev)
    }
}

impl fmt::Display for TenantEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for TenantEnv {
    type Err = TenantEnvError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "production" | "prod" => Ok(Self::Production),
            "preview" | "staging" => Ok(Self::Preview),
            "dev" | "development" | "local" => Ok(Self::Dev),
            _ => Err(TenantEnvError(s.to_string())),
        }
    }
}

/// Error when parsing an invalid tenant environment.
#[derive(Debug, Clone)]
pub struct TenantEnvError(String);

impl fmt::Display for TenantEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid tenant env '{}': expected 'production', 'preview', or 'dev'",
            self.0
        )
    }
}

impl std::error::Error for TenantEnvError {}

/// Tenant context — per-request tenant information.
///
/// This struct is injected into request extensions by the tenant middleware
/// and extracted by capability handlers to scope their operations.
///
/// # Single-tenant mode (G2)
///
/// In single-tenant deployments, the `TenantCtx` is constructed once at
/// boot time from configuration and cloned for every request.
///
/// # Multi-tenant mode (G3c)
///
/// In multi-tenant deployments, the `TenantCtx` is resolved per-request
/// from the Host header (subdomain) or JWT claims, and adapters are
/// opened/cached per tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantCtx {
    /// Immutable project identifier (UUID).
    project_id: ProjectId,

    /// URL-safe project reference (subdomain).
    project_ref: ProjectRef,

    /// Human-readable project name.
    project_name: String,

    /// Deployment environment.
    env: TenantEnv,
}

impl TenantCtx {
    /// Create a new tenant context.
    #[must_use]
    pub fn new(
        project_id: ProjectId,
        project_ref: ProjectRef,
        project_name: impl Into<String>,
        env: TenantEnv,
    ) -> Self {
        Self {
            project_id,
            project_ref,
            project_name: project_name.into(),
            env,
        }
    }

    /// Create a tenant context from just a project ID and name.
    ///
    /// The project ref is derived automatically, and env defaults to Production.
    #[must_use]
    pub fn from_project_id(project_id: ProjectId, project_name: impl Into<String>) -> Self {
        let project_ref = project_id.to_ref();
        Self {
            project_id,
            project_ref,
            project_name: project_name.into(),
            env: TenantEnv::Production,
        }
    }

    /// Get the project ID.
    #[must_use]
    pub const fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    /// Get the project ref (subdomain).
    #[must_use]
    pub fn project_ref(&self) -> &ProjectRef {
        &self.project_ref
    }

    /// Get the project name.
    #[must_use]
    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    /// Get the environment.
    #[must_use]
    pub const fn env(&self) -> TenantEnv {
        self.env
    }

    /// Check if this is a production tenant.
    #[must_use]
    pub const fn is_production(&self) -> bool {
        self.env.is_production()
    }

    /// Create a nil/empty tenant context for testing.
    #[must_use]
    pub fn nil() -> Self {
        let project_id = ProjectId::nil();
        Self {
            project_id,
            project_ref: project_id.to_ref(),
            project_name: String::new(),
            env: TenantEnv::Dev,
        }
    }

    /// Wrap in an Arc for cheap cloning in handlers.
    #[must_use]
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

impl fmt::Display for TenantCtx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TenantCtx({} / {} / {})",
            self.project_ref, self.project_name, self.env
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_env_parse() {
        assert_eq!(TenantEnv::from_str("production").unwrap(), TenantEnv::Production);
        assert_eq!(TenantEnv::from_str("prod").unwrap(), TenantEnv::Production);
        assert_eq!(TenantEnv::from_str("preview").unwrap(), TenantEnv::Preview);
        assert_eq!(TenantEnv::from_str("staging").unwrap(), TenantEnv::Preview);
        assert_eq!(TenantEnv::from_str("dev").unwrap(), TenantEnv::Dev);
        assert_eq!(TenantEnv::from_str("development").unwrap(), TenantEnv::Dev);
        assert_eq!(TenantEnv::from_str("local").unwrap(), TenantEnv::Dev);
        assert!(TenantEnv::from_str("invalid").is_err());
    }

    #[test]
    fn test_tenant_env_display() {
        assert_eq!(TenantEnv::Production.to_string(), "production");
        assert_eq!(TenantEnv::Preview.to_string(), "preview");
        assert_eq!(TenantEnv::Dev.to_string(), "dev");
    }

    #[test]
    fn test_tenant_ctx_new() {
        let id = ProjectId::new();
        let ref_ = id.to_ref();
        let ctx = TenantCtx::new(id, ref_.clone(), "My Project", TenantEnv::Production);

        assert_eq!(ctx.project_id(), &id);
        assert_eq!(ctx.project_ref(), &ref_);
        assert_eq!(ctx.project_name(), "My Project");
        assert_eq!(ctx.env(), TenantEnv::Production);
        assert!(ctx.is_production());
    }

    #[test]
    fn test_tenant_ctx_from_project_id() {
        let id = ProjectId::new();
        let ctx = TenantCtx::from_project_id(id, "Test Project");

        assert_eq!(ctx.project_id(), &id);
        assert_eq!(ctx.project_ref(), &id.to_ref());
        assert_eq!(ctx.project_name(), "Test Project");
        assert_eq!(ctx.env(), TenantEnv::Production);
    }

    #[test]
    fn test_tenant_ctx_nil() {
        let ctx = TenantCtx::nil();
        assert!(ctx.project_id().is_nil());
        assert!(ctx.project_name().is_empty());
        assert_eq!(ctx.env(), TenantEnv::Dev);
    }

    #[test]
    fn test_tenant_ctx_display() {
        let id = ProjectId::new();
        let ctx = TenantCtx::from_project_id(id, "Test");
        let display = ctx.to_string();
        assert!(display.contains("TenantCtx"));
        assert!(display.contains("Test"));
        assert!(display.contains("production"));
    }

    #[test]
    fn test_tenant_ctx_serde() {
        let id = ProjectId::new();
        let ctx = TenantCtx::from_project_id(id, "Serde Test");
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: TenantCtx = serde_json::from_str(&json).unwrap();

        assert_eq!(ctx.project_id(), parsed.project_id());
        assert_eq!(ctx.project_ref(), parsed.project_ref());
        assert_eq!(ctx.project_name(), parsed.project_name());
        assert_eq!(ctx.env(), parsed.env());
    }
}
