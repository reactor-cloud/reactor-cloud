//! Audit log service.

use std::sync::Arc;

use crate::error::CloudError;
use crate::store::ProjectStore;
use crate::types::AuditEntry;

/// Audit service for reading audit logs.
pub struct AuditService {
    store: Arc<dyn ProjectStore>,
}

impl AuditService {
    /// Create a new audit service.
    pub fn new(store: Arc<dyn ProjectStore>) -> Self {
        Self { store }
    }

    /// Get audit log entries for a project.
    pub async fn get_for_project(
        &self,
        project_ref: &str,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<AuditEntry>, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        self.store
            .get_audit_log(Some(&project.project_id()), limit, offset)
            .await
    }

    /// Get all audit log entries (admin).
    pub async fn get_all(&self, limit: i32, offset: i32) -> Result<Vec<AuditEntry>, CloudError> {
        self.store.get_audit_log(None, limit, offset).await
    }
}
