//! Core types for the cloud control plane.

use chrono::{DateTime, Utc};
use reactor_core::{ProjectId, ProjectRef};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Project status in the lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    /// Project is being provisioned.
    Provisioning,
    /// Project is active and serving requests.
    Active,
    /// Project is suspended (billing, abuse, etc.).
    Suspended,
    /// Project is being deleted.
    Deleting,
    /// Provisioning failed.
    Failed,
}

impl ProjectStatus {
    /// Returns the status as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Deleting => "deleting",
            Self::Failed => "failed",
        }
    }

    /// Check if the project can accept requests.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Check if the project is in a terminal failure state.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed)
    }

    /// Check if the project can be deleted.
    pub fn can_delete(&self) -> bool {
        matches!(self, Self::Active | Self::Suspended | Self::Failed)
    }
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ProjectStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "provisioning" => Ok(Self::Provisioning),
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "deleting" => Ok(Self::Deleting),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("invalid project status: {}", s)),
        }
    }
}

/// Backend kind for the project deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    /// Dedicated single-node backend (Phase 3).
    Dedicated,
    /// Shared cluster backend (Phase 4+).
    Shared,
}

impl BackendKind {
    /// Returns the backend kind as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dedicated => "dedicated",
            Self::Shared => "shared",
        }
    }
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A Reactor Cloud project.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    /// Unique project identifier.
    pub id: Uuid,
    /// URL-safe project reference (20 chars).
    #[sqlx(rename = "ref")]
    pub project_ref: String,
    /// Human-readable project name.
    pub name: String,
    /// Owner user ID.
    pub owner_user_id: Uuid,
    /// Backend deployment kind.
    pub backend_kind: String,
    /// Current status.
    pub status: String,
    /// Deployment region.
    pub region: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl Project {
    /// Get the project ID as a typed ProjectId.
    pub fn project_id(&self) -> ProjectId {
        ProjectId::from(self.id)
    }

    /// Get the project ref as a typed ProjectRef.
    pub fn project_ref_typed(&self) -> ProjectRef {
        ProjectRef::from_string_unchecked(self.project_ref.clone())
    }

    /// Get the parsed status.
    pub fn parsed_status(&self) -> ProjectStatus {
        self.status.parse().unwrap_or(ProjectStatus::Failed)
    }

    /// Get the parsed backend kind.
    pub fn parsed_backend_kind(&self) -> BackendKind {
        match self.backend_kind.as_str() {
            "shared" => BackendKind::Shared,
            _ => BackendKind::Dedicated,
        }
    }

    /// Get the schema name for this project.
    pub fn schema_name(&self) -> String {
        format!("tenant_{}", self.project_ref)
    }

    /// Get the hostname for this project using the given base domain.
    pub fn hostname_for(&self, base_domain: &str) -> String {
        format!("{}.{}", self.project_ref, base_domain)
    }

    /// Get the hostname for this project (uses default reactor.cloud domain).
    /// 
    /// Deprecated: Use `hostname_for(base_domain)` instead for multi-domain support.
    pub fn hostname(&self) -> String {
        self.hostname_for("reactor.cloud")
    }
}

/// Member role in a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    /// Project owner (full control).
    Owner,
    /// Administrator (most operations).
    Admin,
    /// Regular member (read + limited write).
    Member,
}

impl MemberRole {
    /// Returns the role as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    /// Check if this role can manage members.
    pub fn can_manage_members(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    /// Check if this role can manage keys.
    pub fn can_manage_keys(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    /// Check if this role can delete the project.
    pub fn can_delete_project(&self) -> bool {
        matches!(self, Self::Owner)
    }
}

impl std::fmt::Display for MemberRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for MemberRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            _ => Err(format!("invalid member role: {}", s)),
        }
    }
}

/// A project member.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProjectMember {
    /// Project ID.
    pub project_id: Uuid,
    /// User ID.
    pub user_id: Uuid,
    /// Member role.
    pub role: String,
    /// When the membership was created.
    pub created_at: DateTime<Utc>,
}

impl ProjectMember {
    /// Get the parsed role.
    pub fn parsed_role(&self) -> MemberRole {
        self.role.parse().unwrap_or(MemberRole::Member)
    }
}

/// Key kind for project API keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyKind {
    /// Anonymous key (public, read-only).
    Anon,
    /// Service key (server-side, full access).
    Service,
    /// JWT signing key (internal).
    JwtSigning,
}

impl KeyKind {
    /// Returns the key kind as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anon => "anon",
            Self::Service => "service",
            Self::JwtSigning => "jwt-signing",
        }
    }
}

impl std::fmt::Display for KeyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for KeyKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anon" => Ok(Self::Anon),
            "service" => Ok(Self::Service),
            "jwt-signing" => Ok(Self::JwtSigning),
            _ => Err(format!("invalid key kind: {}", s)),
        }
    }
}

/// A project API key.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProjectKey {
    /// Unique key identifier.
    pub id: Uuid,
    /// Project ID.
    pub project_id: Uuid,
    /// Key kind.
    pub kind: String,
    /// Vault reference path.
    pub vault_ref: String,
    /// When the key was revoked (if revoked).
    pub revoked_at: Option<DateTime<Utc>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl ProjectKey {
    /// Get the parsed key kind.
    pub fn parsed_kind(&self) -> KeyKind {
        self.kind.parse().unwrap_or(KeyKind::Anon)
    }

    /// Check if the key is active (not revoked).
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }
}

/// Audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEntry {
    /// Entry ID.
    pub id: i64,
    /// Project ID (null for global events).
    pub project_id: Option<Uuid>,
    /// Actor identifier (user ID, system, etc.).
    pub actor: String,
    /// Action performed.
    pub action: String,
    /// Additional metadata.
    pub metadata: serde_json::Value,
    /// Timestamp.
    pub created_at: DateTime<Utc>,
}

/// Audit log actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    /// Project was created.
    ProjectCreated,
    /// Project provisioning completed.
    ProjectProvisioned,
    /// Project provisioning failed.
    ProjectProvisionFailed,
    /// Project was suspended.
    ProjectSuspended,
    /// Project was resumed from suspension.
    ProjectResumed,
    /// Project deletion was scheduled.
    ProjectDeleteScheduled,
    /// Project was fully deleted.
    ProjectDeleted,
    /// Member was added to project.
    MemberAdded,
    /// Member was removed from project.
    MemberRemoved,
    /// Member role was changed.
    MemberRoleChanged,
    /// API key was created.
    KeyCreated,
    /// API key was rotated.
    KeyRotated,
    /// API key was revoked.
    KeyRevoked,
}

impl AuditAction {
    /// Returns the audit action as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProjectCreated => "project.created",
            Self::ProjectProvisioned => "project.provisioned",
            Self::ProjectProvisionFailed => "project.provision_failed",
            Self::ProjectSuspended => "project.suspended",
            Self::ProjectResumed => "project.resumed",
            Self::ProjectDeleteScheduled => "project.delete_scheduled",
            Self::ProjectDeleted => "project.deleted",
            Self::MemberAdded => "member.added",
            Self::MemberRemoved => "member.removed",
            Self::MemberRoleChanged => "member.role_changed",
            Self::KeyCreated => "key.created",
            Self::KeyRotated => "key.rotated",
            Self::KeyRevoked => "key.revoked",
        }
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Specification for creating a new project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSpec {
    /// Project ID (generated).
    pub project_id: ProjectId,
    /// Project ref (derived from ID).
    pub project_ref: ProjectRef,
    /// Human-readable name.
    pub name: String,
    /// Deployment region.
    pub region: String,
    /// Owner user ID.
    pub owner_user_id: Uuid,
}

/// Result of provisioning a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResult {
    /// Backend target for gateway routing.
    pub backend_target: String,
    /// Anon API key (returned once at creation).
    pub anon_key: String,
    /// Service API key (returned once at creation).
    pub service_key: String,
}

/// Project health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHealth {
    /// Overall health status.
    pub healthy: bool,
    /// Schema exists.
    pub schema_exists: bool,
    /// Vault keys accessible.
    pub vault_accessible: bool,
    /// Route configured.
    pub route_configured: bool,
    /// Optional error message.
    pub error: Option<String>,
}
