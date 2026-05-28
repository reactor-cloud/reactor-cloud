//! Persistent storage for cloud control plane data.
//!
//! The [`ProjectStore`] trait defines operations for managing projects, members,
//! keys, and audit logs. The default implementation uses PostgreSQL.

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::error::CloudError;
use crate::types::{
    AuditAction, AuditEntry, MemberRole, Project, ProjectKey, ProjectMember, ProjectStatus,
};
use reactor_core::ProjectId;

/// Trait for project storage operations.
#[async_trait]
pub trait ProjectStore: Send + Sync {
    // =========================================================================
    // Project operations
    // =========================================================================

    /// Create a new project.
    async fn create_project(
        &self,
        id: Uuid,
        project_ref: &str,
        name: &str,
        owner_user_id: Uuid,
        region: &str,
    ) -> Result<Project, CloudError>;

    /// Get a project by ID.
    async fn get_project_by_id(&self, id: &ProjectId) -> Result<Option<Project>, CloudError>;

    /// Get a project by ref.
    async fn get_project_by_ref(&self, project_ref: &str) -> Result<Option<Project>, CloudError>;

    /// List projects for a user.
    async fn list_projects_for_user(
        &self,
        user_id: Uuid,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Project>, CloudError>;

    /// List all projects (admin).
    async fn list_all_projects(&self, limit: i32, offset: i32) -> Result<Vec<Project>, CloudError>;

    /// Update project status.
    async fn update_project_status(
        &self,
        id: &ProjectId,
        status: ProjectStatus,
    ) -> Result<(), CloudError>;

    /// Get projects by status (for bootstrap/teardown).
    async fn get_projects_by_status(
        &self,
        status: ProjectStatus,
    ) -> Result<Vec<Project>, CloudError>;

    /// Delete a project record.
    async fn delete_project(&self, id: &ProjectId) -> Result<(), CloudError>;

    // =========================================================================
    // Member operations
    // =========================================================================

    /// Add a member to a project.
    async fn add_member(
        &self,
        project_id: &ProjectId,
        user_id: Uuid,
        role: MemberRole,
    ) -> Result<ProjectMember, CloudError>;

    /// Get a member.
    async fn get_member(
        &self,
        project_id: &ProjectId,
        user_id: Uuid,
    ) -> Result<Option<ProjectMember>, CloudError>;

    /// List project members.
    async fn list_members(&self, project_id: &ProjectId) -> Result<Vec<ProjectMember>, CloudError>;

    /// Update member role.
    async fn update_member_role(
        &self,
        project_id: &ProjectId,
        user_id: Uuid,
        role: MemberRole,
    ) -> Result<(), CloudError>;

    /// Remove a member.
    async fn remove_member(&self, project_id: &ProjectId, user_id: Uuid) -> Result<(), CloudError>;

    // =========================================================================
    // Key operations
    // =========================================================================

    /// Create a project key.
    async fn create_key(
        &self,
        project_id: &ProjectId,
        kind: &str,
        vault_ref: &str,
    ) -> Result<ProjectKey, CloudError>;

    /// Get a key by ID.
    async fn get_key(&self, key_id: Uuid) -> Result<Option<ProjectKey>, CloudError>;

    /// List project keys.
    async fn list_keys(&self, project_id: &ProjectId) -> Result<Vec<ProjectKey>, CloudError>;

    /// Revoke a key.
    async fn revoke_key(&self, key_id: Uuid) -> Result<(), CloudError>;

    // =========================================================================
    // Audit operations
    // =========================================================================

    /// Append an audit log entry.
    async fn audit_log(
        &self,
        project_id: Option<&ProjectId>,
        actor: &str,
        action: AuditAction,
        metadata: serde_json::Value,
    ) -> Result<(), CloudError>;

    /// Get audit log entries.
    async fn get_audit_log(
        &self,
        project_id: Option<&ProjectId>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<AuditEntry>, CloudError>;

    // =========================================================================
    // Route operations
    // =========================================================================

    /// Create a gateway route.
    async fn create_route(
        &self,
        host: &str,
        project_id: &ProjectId,
        project_ref: &str,
        backend_kind: &str,
        backend_target: &str,
        tls_mode: &str,
    ) -> Result<(), CloudError>;

    /// Get a route by host.
    async fn get_route(&self, host: &str) -> Result<Option<RouteInfo>, CloudError>;

    /// Delete a route.
    async fn delete_route(&self, host: &str) -> Result<(), CloudError>;
}

/// Route information from the gateway table.
#[derive(Debug, Clone)]
pub struct RouteInfo {
    /// Hostname for this route.
    pub host: String,
    /// Project ID this route belongs to.
    pub project_id: Uuid,
    /// Project ref for this route.
    pub project_ref: String,
    /// Backend kind (dedicated/shared).
    pub backend_kind: String,
    /// Backend target address.
    pub backend_target: String,
    /// TLS mode for this route.
    pub tls_mode: String,
    /// Whether the route is enabled.
    pub enabled: bool,
}

/// PostgreSQL implementation of the project store.
pub struct PgProjectStore {
    pool: PgPool,
}

impl PgProjectStore {
    /// Create a new PostgreSQL project store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectStore for PgProjectStore {
    #[instrument(skip(self))]
    async fn create_project(
        &self,
        id: Uuid,
        project_ref: &str,
        name: &str,
        owner_user_id: Uuid,
        region: &str,
    ) -> Result<Project, CloudError> {
        debug!(project_ref = %project_ref, "creating project");

        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO reactor_cloud.projects (id, ref, name, owner_user_id, region)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, ref, name, owner_user_id, backend_kind, status, region, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(project_ref)
        .bind(name)
        .bind(owner_user_id)
        .bind(region)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("duplicate key") {
                CloudError::ProjectAlreadyExists(project_ref.to_string())
            } else {
                CloudError::Database(e)
            }
        })?;

        // Add owner as a member
        self.add_member(&ProjectId::from(id), owner_user_id, MemberRole::Owner)
            .await?;

        Ok(project)
    }

    async fn get_project_by_id(&self, id: &ProjectId) -> Result<Option<Project>, CloudError> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            SELECT id, ref, name, owner_user_id, backend_kind, status, region, created_at, updated_at
            FROM reactor_cloud.projects
            WHERE id = $1
            "#,
        )
        .bind(Uuid::from(*id))
        .fetch_optional(&self.pool)
        .await?;

        Ok(project)
    }

    async fn get_project_by_ref(&self, project_ref: &str) -> Result<Option<Project>, CloudError> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            SELECT id, ref, name, owner_user_id, backend_kind, status, region, created_at, updated_at
            FROM reactor_cloud.projects
            WHERE ref = $1
            "#,
        )
        .bind(project_ref)
        .fetch_optional(&self.pool)
        .await?;

        Ok(project)
    }

    async fn list_projects_for_user(
        &self,
        user_id: Uuid,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Project>, CloudError> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.id, p.ref, p.name, p.owner_user_id, p.backend_kind, p.status, p.region, p.created_at, p.updated_at
            FROM reactor_cloud.projects p
            JOIN reactor_cloud.project_members m ON p.id = m.project_id
            WHERE m.user_id = $1
            ORDER BY p.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(projects)
    }

    async fn list_all_projects(&self, limit: i32, offset: i32) -> Result<Vec<Project>, CloudError> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT id, ref, name, owner_user_id, backend_kind, status, region, created_at, updated_at
            FROM reactor_cloud.projects
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(projects)
    }

    async fn update_project_status(
        &self,
        id: &ProjectId,
        status: ProjectStatus,
    ) -> Result<(), CloudError> {
        sqlx::query(
            r#"
            UPDATE reactor_cloud.projects
            SET status = $1, updated_at = now()
            WHERE id = $2
            "#,
        )
        .bind(status.as_str())
        .bind(Uuid::from(*id))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_projects_by_status(
        &self,
        status: ProjectStatus,
    ) -> Result<Vec<Project>, CloudError> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT id, ref, name, owner_user_id, backend_kind, status, region, created_at, updated_at
            FROM reactor_cloud.projects
            WHERE status = $1
            "#,
        )
        .bind(status.as_str())
        .fetch_all(&self.pool)
        .await?;

        Ok(projects)
    }

    async fn delete_project(&self, id: &ProjectId) -> Result<(), CloudError> {
        sqlx::query(
            r#"
            DELETE FROM reactor_cloud.projects WHERE id = $1
            "#,
        )
        .bind(Uuid::from(*id))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn add_member(
        &self,
        project_id: &ProjectId,
        user_id: Uuid,
        role: MemberRole,
    ) -> Result<ProjectMember, CloudError> {
        let member = sqlx::query_as::<_, ProjectMember>(
            r#"
            INSERT INTO reactor_cloud.project_members (project_id, user_id, role)
            VALUES ($1, $2, $3)
            RETURNING project_id, user_id, role, created_at
            "#,
        )
        .bind(Uuid::from(*project_id))
        .bind(user_id)
        .bind(role.as_str())
        .fetch_one(&self.pool)
        .await?;

        Ok(member)
    }

    async fn get_member(
        &self,
        project_id: &ProjectId,
        user_id: Uuid,
    ) -> Result<Option<ProjectMember>, CloudError> {
        let member = sqlx::query_as::<_, ProjectMember>(
            r#"
            SELECT project_id, user_id, role, created_at
            FROM reactor_cloud.project_members
            WHERE project_id = $1 AND user_id = $2
            "#,
        )
        .bind(Uuid::from(*project_id))
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(member)
    }

    async fn list_members(&self, project_id: &ProjectId) -> Result<Vec<ProjectMember>, CloudError> {
        let members = sqlx::query_as::<_, ProjectMember>(
            r#"
            SELECT project_id, user_id, role, created_at
            FROM reactor_cloud.project_members
            WHERE project_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(Uuid::from(*project_id))
        .fetch_all(&self.pool)
        .await?;

        Ok(members)
    }

    async fn update_member_role(
        &self,
        project_id: &ProjectId,
        user_id: Uuid,
        role: MemberRole,
    ) -> Result<(), CloudError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_cloud.project_members
            SET role = $1
            WHERE project_id = $2 AND user_id = $3
            "#,
        )
        .bind(role.as_str())
        .bind(Uuid::from(*project_id))
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CloudError::MemberNotFound {
                project_id: Uuid::from(*project_id),
                user_id,
            });
        }

        Ok(())
    }

    async fn remove_member(&self, project_id: &ProjectId, user_id: Uuid) -> Result<(), CloudError> {
        sqlx::query(
            r#"
            DELETE FROM reactor_cloud.project_members
            WHERE project_id = $1 AND user_id = $2
            "#,
        )
        .bind(Uuid::from(*project_id))
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn create_key(
        &self,
        project_id: &ProjectId,
        kind: &str,
        vault_ref: &str,
    ) -> Result<ProjectKey, CloudError> {
        let key = sqlx::query_as::<_, ProjectKey>(
            r#"
            INSERT INTO reactor_cloud.project_keys (id, project_id, kind, vault_ref)
            VALUES ($1, $2, $3, $4)
            RETURNING id, project_id, kind, vault_ref, revoked_at, created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(Uuid::from(*project_id))
        .bind(kind)
        .bind(vault_ref)
        .fetch_one(&self.pool)
        .await?;

        Ok(key)
    }

    async fn get_key(&self, key_id: Uuid) -> Result<Option<ProjectKey>, CloudError> {
        let key = sqlx::query_as::<_, ProjectKey>(
            r#"
            SELECT id, project_id, kind, vault_ref, revoked_at, created_at
            FROM reactor_cloud.project_keys
            WHERE id = $1
            "#,
        )
        .bind(key_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    async fn list_keys(&self, project_id: &ProjectId) -> Result<Vec<ProjectKey>, CloudError> {
        let keys = sqlx::query_as::<_, ProjectKey>(
            r#"
            SELECT id, project_id, kind, vault_ref, revoked_at, created_at
            FROM reactor_cloud.project_keys
            WHERE project_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(Uuid::from(*project_id))
        .fetch_all(&self.pool)
        .await?;

        Ok(keys)
    }

    async fn revoke_key(&self, key_id: Uuid) -> Result<(), CloudError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_cloud.project_keys
            SET revoked_at = now()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(key_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CloudError::KeyNotFound(key_id));
        }

        Ok(())
    }

    async fn audit_log(
        &self,
        project_id: Option<&ProjectId>,
        actor: &str,
        action: AuditAction,
        metadata: serde_json::Value,
    ) -> Result<(), CloudError> {
        sqlx::query(
            r#"
            INSERT INTO reactor_cloud.audit_log (project_id, actor, action, metadata)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(project_id.map(|id| Uuid::from(*id)))
        .bind(actor)
        .bind(action.as_str())
        .bind(metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_audit_log(
        &self,
        project_id: Option<&ProjectId>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<AuditEntry>, CloudError> {
        let entries = match project_id {
            Some(pid) => {
                sqlx::query_as::<_, AuditEntry>(
                    r#"
                    SELECT id, project_id, actor, action, metadata, created_at
                    FROM reactor_cloud.audit_log
                    WHERE project_id = $1
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#,
                )
                .bind(Uuid::from(*pid))
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, AuditEntry>(
                    r#"
                    SELECT id, project_id, actor, action, metadata, created_at
                    FROM reactor_cloud.audit_log
                    ORDER BY created_at DESC
                    LIMIT $1 OFFSET $2
                    "#,
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
            }
        };

        Ok(entries)
    }

    async fn create_route(
        &self,
        host: &str,
        project_id: &ProjectId,
        project_ref: &str,
        backend_kind: &str,
        backend_target: &str,
        tls_mode: &str,
    ) -> Result<(), CloudError> {
        sqlx::query(
            r#"
            INSERT INTO reactor_gateway.routes (host, project_id, project_ref, backend_kind, backend_target, tls_mode, enabled)
            VALUES ($1, $2, $3, $4, $5, $6, true)
            ON CONFLICT (host) DO UPDATE SET
                project_id = EXCLUDED.project_id,
                project_ref = EXCLUDED.project_ref,
                backend_kind = EXCLUDED.backend_kind,
                backend_target = EXCLUDED.backend_target,
                tls_mode = EXCLUDED.tls_mode,
                enabled = true,
                updated_at = now()
            "#,
        )
        .bind(host)
        .bind(Uuid::from(*project_id))
        .bind(project_ref)
        .bind(backend_kind)
        .bind(backend_target)
        .bind(tls_mode)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_route(&self, host: &str) -> Result<Option<RouteInfo>, CloudError> {
        let route = sqlx::query_as::<_, (String, Uuid, String, String, String, String, bool)>(
            r#"
            SELECT host, project_id, project_ref, backend_kind, backend_target, tls_mode, enabled
            FROM reactor_gateway.routes
            WHERE host = $1
            "#,
        )
        .bind(host)
        .fetch_optional(&self.pool)
        .await?
        .map(
            |(host, project_id, project_ref, backend_kind, backend_target, tls_mode, enabled)| {
                RouteInfo {
                    host,
                    project_id,
                    project_ref,
                    backend_kind,
                    backend_target,
                    tls_mode,
                    enabled,
                }
            },
        );

        Ok(route)
    }

    async fn delete_route(&self, host: &str) -> Result<(), CloudError> {
        sqlx::query(
            r#"
            DELETE FROM reactor_gateway.routes WHERE host = $1
            "#,
        )
        .bind(host)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
