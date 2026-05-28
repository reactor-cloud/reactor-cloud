//! Project member management service.

use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

use crate::error::CloudError;
use crate::store::ProjectStore;
use crate::types::{AuditAction, MemberRole, ProjectMember};

/// Member service for managing project membership.
pub struct MemberService {
    store: Arc<dyn ProjectStore>,
}

impl MemberService {
    /// Create a new member service.
    pub fn new(store: Arc<dyn ProjectStore>) -> Self {
        Self { store }
    }

    /// Add a member to a project.
    #[instrument(skip(self))]
    pub async fn add(
        &self,
        project_ref: &str,
        user_id: Uuid,
        role: MemberRole,
        actor: &str,
    ) -> Result<ProjectMember, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let project_id = project.project_id();

        // Owners cannot be added (only during project creation)
        if role == MemberRole::Owner {
            return Err(CloudError::InvalidArgument(
                "cannot add owner role - owners are set at project creation".to_string(),
            ));
        }

        let member = self.store.add_member(&project_id, user_id, role).await?;

        // Audit log
        self.store
            .audit_log(
                Some(&project_id),
                actor,
                AuditAction::MemberAdded,
                serde_json::json!({
                    "user_id": user_id,
                    "role": role.as_str(),
                }),
            )
            .await?;

        Ok(member)
    }

    /// List members of a project.
    pub async fn list(&self, project_ref: &str) -> Result<Vec<ProjectMember>, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        self.store.list_members(&project.project_id()).await
    }

    /// Update a member's role.
    #[instrument(skip(self))]
    pub async fn update_role(
        &self,
        project_ref: &str,
        user_id: Uuid,
        role: MemberRole,
        actor: &str,
    ) -> Result<(), CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let project_id = project.project_id();

        // Cannot change owner role
        if role == MemberRole::Owner {
            return Err(CloudError::InvalidArgument(
                "cannot change role to owner".to_string(),
            ));
        }

        // Get current member to check if they're the owner
        let member = self
            .store
            .get_member(&project_id, user_id)
            .await?
            .ok_or_else(|| CloudError::MemberNotFound {
                project_id: Uuid::from(project_id),
                user_id,
            })?;

        if member.parsed_role() == MemberRole::Owner {
            return Err(CloudError::InvalidArgument(
                "cannot change owner's role".to_string(),
            ));
        }

        self.store
            .update_member_role(&project_id, user_id, role)
            .await?;

        // Audit log
        self.store
            .audit_log(
                Some(&project_id),
                actor,
                AuditAction::MemberRoleChanged,
                serde_json::json!({
                    "user_id": user_id,
                    "new_role": role.as_str(),
                }),
            )
            .await?;

        Ok(())
    }

    /// Remove a member from a project.
    #[instrument(skip(self))]
    pub async fn remove(
        &self,
        project_ref: &str,
        user_id: Uuid,
        actor: &str,
    ) -> Result<(), CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let project_id = project.project_id();

        // Cannot remove owner
        let member = self
            .store
            .get_member(&project_id, user_id)
            .await?
            .ok_or_else(|| CloudError::MemberNotFound {
                project_id: Uuid::from(project_id),
                user_id,
            })?;

        if member.parsed_role() == MemberRole::Owner {
            return Err(CloudError::InvalidArgument(
                "cannot remove project owner".to_string(),
            ));
        }

        self.store.remove_member(&project_id, user_id).await?;

        // Audit log
        self.store
            .audit_log(
                Some(&project_id),
                actor,
                AuditAction::MemberRemoved,
                serde_json::json!({
                    "user_id": user_id,
                }),
            )
            .await?;

        Ok(())
    }

    /// Check if a user has access to a project.
    pub async fn has_access(&self, project_ref: &str, user_id: Uuid) -> Result<bool, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let member = self.store.get_member(&project.project_id(), user_id).await?;
        Ok(member.is_some())
    }

    /// Check if a user has a specific role or higher.
    pub async fn has_role(
        &self,
        project_ref: &str,
        user_id: Uuid,
        required_role: MemberRole,
    ) -> Result<bool, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let member = self
            .store
            .get_member(&project.project_id(), user_id)
            .await?;

        match member {
            Some(m) => {
                let role = m.parsed_role();
                // Owner > Admin > Member
                let has_required = match required_role {
                    MemberRole::Member => true,
                    MemberRole::Admin => role == MemberRole::Admin || role == MemberRole::Owner,
                    MemberRole::Owner => role == MemberRole::Owner,
                };
                Ok(has_required)
            }
            None => Ok(false),
        }
    }
}
