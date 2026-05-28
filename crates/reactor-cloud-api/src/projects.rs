//! Project management service.
//!
//! Handles project CRUD operations, provisioning, and lifecycle management.

use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::error::CloudError;
use crate::provisioner::CloudProvider;
use crate::store::ProjectStore;
use crate::types::{AuditAction, Project, ProjectSpec, ProjectStatus};
use reactor_core::ProjectId;

/// Request to create a new project.
#[derive(Debug, Clone)]
pub struct CreateProjectRequest {
    /// Human-readable name.
    pub name: String,
    /// Deployment region (default: "iad").
    pub region: Option<String>,
    /// Owner user ID.
    pub owner_user_id: Uuid,
}

/// Result of creating a project.
#[derive(Debug, Clone)]
pub struct CreateProjectResult {
    /// The created project.
    pub project: Project,
    /// Anon API key (only returned once).
    pub anon_key: String,
    /// Service API key (only returned once).
    pub service_key: String,
}

/// Project service for managing project lifecycle.
pub struct ProjectService {
    store: Arc<dyn ProjectStore>,
    provider: Arc<dyn CloudProvider>,
}

impl ProjectService {
    /// Create a new project service.
    pub fn new(store: Arc<dyn ProjectStore>, provider: Arc<dyn CloudProvider>) -> Self {
        Self { store, provider }
    }

    /// Create a new project.
    ///
    /// This creates the project record, provisions infrastructure, and returns
    /// the initial API keys. The keys are only returned once at creation time.
    #[instrument(skip(self))]
    pub async fn create(&self, request: CreateProjectRequest) -> Result<CreateProjectResult, CloudError> {
        let project_id = ProjectId::new();
        let project_ref = project_id.to_ref();
        let region = request.region.unwrap_or_else(|| "iad".to_string());

        info!(
            project_id = %project_id,
            project_ref = %project_ref,
            name = %request.name,
            "creating project"
        );

        // Create project record in provisioning state
        let _project = self
            .store
            .create_project(
                Uuid::from(project_id),
                project_ref.as_str(),
                &request.name,
                request.owner_user_id,
                &region,
            )
            .await?;

        // Audit log
        self.store
            .audit_log(
                Some(&project_id),
                &request.owner_user_id.to_string(),
                AuditAction::ProjectCreated,
                serde_json::json!({
                    "name": request.name,
                    "region": region,
                }),
            )
            .await?;

        // Provision infrastructure
        let spec = ProjectSpec {
            project_id,
            project_ref: project_ref.clone(),
            name: request.name.clone(),
            region: region.clone(),
            owner_user_id: request.owner_user_id,
        };

        match self.provider.provision(&spec).await {
            Ok(provision_result) => {
                // Update status to active
                self.store
                    .update_project_status(&project_id, ProjectStatus::Active)
                    .await?;

                // Audit log
                self.store
                    .audit_log(
                        Some(&project_id),
                        "system",
                        AuditAction::ProjectProvisioned,
                        serde_json::json!({
                            "backend_target": provision_result.backend_target,
                        }),
                    )
                    .await?;

                // Re-fetch project with updated status
                let project = self
                    .store
                    .get_project_by_id(&project_id)
                    .await?
                    .ok_or_else(|| CloudError::Internal("project disappeared".to_string()))?;

                info!(
                    project_id = %project_id,
                    project_ref = %project_ref,
                    "project created and provisioned"
                );

                Ok(CreateProjectResult {
                    project,
                    anon_key: provision_result.anon_key,
                    service_key: provision_result.service_key,
                })
            }
            Err(e) => {
                error!(
                    project_id = %project_id,
                    error = %e,
                    "provisioning failed"
                );

                // Update status to failed
                self.store
                    .update_project_status(&project_id, ProjectStatus::Failed)
                    .await?;

                // Audit log
                self.store
                    .audit_log(
                        Some(&project_id),
                        "system",
                        AuditAction::ProjectProvisionFailed,
                        serde_json::json!({
                            "error": e.to_string(),
                        }),
                    )
                    .await?;

                Err(CloudError::ProvisioningFailed(e.to_string()))
            }
        }
    }

    /// Get a project by ref.
    pub async fn get_by_ref(&self, project_ref: &str) -> Result<Option<Project>, CloudError> {
        self.store.get_project_by_ref(project_ref).await
    }

    /// Get a project by ID.
    pub async fn get_by_id(&self, project_id: &ProjectId) -> Result<Option<Project>, CloudError> {
        self.store.get_project_by_id(project_id).await
    }

    /// List projects for a user.
    pub async fn list_for_user(
        &self,
        user_id: Uuid,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Project>, CloudError> {
        self.store.list_projects_for_user(user_id, limit, offset).await
    }

    /// List all projects (admin).
    pub async fn list_all(&self, limit: i32, offset: i32) -> Result<Vec<Project>, CloudError> {
        self.store.list_all_projects(limit, offset).await
    }

    /// Schedule project deletion.
    ///
    /// Sets the project to 'deleting' status. Background task will handle
    /// actual teardown and deletion.
    #[instrument(skip(self))]
    pub async fn schedule_delete(
        &self,
        project_ref: &str,
        actor: &str,
    ) -> Result<Project, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let status = project.parsed_status();
        if !status.can_delete() {
            return Err(CloudError::InvalidStatusTransition {
                from: status.to_string(),
                to: "deleting".to_string(),
            });
        }

        let project_id = project.project_id();

        // Update status
        self.store
            .update_project_status(&project_id, ProjectStatus::Deleting)
            .await?;

        // Audit log
        self.store
            .audit_log(
                Some(&project_id),
                actor,
                AuditAction::ProjectDeleteScheduled,
                serde_json::json!({}),
            )
            .await?;

        info!(project_ref = %project_ref, "project deletion scheduled");

        // Return updated project
        self.store
            .get_project_by_id(&project_id)
            .await?
            .ok_or_else(|| CloudError::Internal("project disappeared".to_string()))
    }

    /// Execute teardown for a project.
    ///
    /// Called by the background task to actually tear down infrastructure.
    #[instrument(skip(self))]
    pub async fn execute_teardown(&self, project_id: &ProjectId) -> Result<(), CloudError> {
        info!(project_id = %project_id, "executing project teardown");

        // Tear down infrastructure
        self.provider
            .teardown(project_id)
            .await
            .map_err(|e| CloudError::TeardownFailed(e.to_string()))?;

        // Delete project record
        self.store.delete_project(project_id).await?;

        // Audit log
        self.store
            .audit_log(
                Some(project_id),
                "system",
                AuditAction::ProjectDeleted,
                serde_json::json!({}),
            )
            .await?;

        info!(project_id = %project_id, "project teardown complete");
        Ok(())
    }

    /// Resume provisioning for projects stuck in 'provisioning' state.
    ///
    /// Called on server startup.
    pub async fn resume_provisioning(&self) -> Result<usize, CloudError> {
        let projects = self
            .store
            .get_projects_by_status(ProjectStatus::Provisioning)
            .await?;

        let count = projects.len();
        if count > 0 {
            info!(count = count, "resuming provisioning for stuck projects");
        }

        for project in projects {
            let project_id = project.project_id();
            let project_ref = project.project_ref_typed();

            info!(project_id = %project_id, "resuming provisioning");

            let spec = ProjectSpec {
                project_id,
                project_ref,
                name: project.name.clone(),
                region: project.region.clone(),
                owner_user_id: project.owner_user_id,
            };

            match self.provider.provision(&spec).await {
                Ok(_) => {
                    self.store
                        .update_project_status(&project_id, ProjectStatus::Active)
                        .await?;
                    info!(project_id = %project_id, "provisioning resumed successfully");
                }
                Err(e) => {
                    error!(project_id = %project_id, error = %e, "provisioning failed on resume");
                    self.store
                        .update_project_status(&project_id, ProjectStatus::Failed)
                        .await?;
                }
            }
        }

        Ok(count)
    }

    /// Resume teardown for projects stuck in 'deleting' state.
    ///
    /// Called on server startup.
    pub async fn resume_teardown(&self) -> Result<usize, CloudError> {
        let projects = self
            .store
            .get_projects_by_status(ProjectStatus::Deleting)
            .await?;

        let count = projects.len();
        if count > 0 {
            info!(count = count, "resuming teardown for stuck projects");
        }

        for project in projects {
            let project_id = project.project_id();
            info!(project_id = %project_id, "resuming teardown");

            if let Err(e) = self.execute_teardown(&project_id).await {
                warn!(project_id = %project_id, error = %e, "teardown failed on resume");
            }
        }

        Ok(count)
    }
}
