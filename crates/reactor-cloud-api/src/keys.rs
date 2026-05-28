//! Project API key management service.

use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

use crate::bootstrap::VaultBootstrap;
use crate::error::CloudError;
use crate::store::ProjectStore;
use crate::types::{AuditAction, KeyKind, ProjectKey};
use reactor_core::Vault;

/// Key service for managing project API keys.
pub struct KeyService {
    store: Arc<dyn ProjectStore>,
    vault: Arc<dyn Vault>,
}

/// Result of creating or rotating a key.
#[derive(Debug, Clone)]
pub struct KeyResult {
    /// Key metadata.
    pub key: ProjectKey,
    /// Key value (only returned once).
    pub value: String,
}

impl KeyService {
    /// Create a new key service.
    pub fn new(store: Arc<dyn ProjectStore>, vault: Arc<dyn Vault>) -> Self {
        Self { store, vault }
    }

    /// Create a new API key.
    #[instrument(skip(self))]
    pub async fn create(
        &self,
        project_ref: &str,
        kind: KeyKind,
        actor: &str,
    ) -> Result<KeyResult, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let project_id = project.project_id();
        let vault_ref = format!("tenant/{}/keys/{}", project_id, kind);

        // Generate key based on kind
        let key_value = match kind {
            KeyKind::Anon => {
                VaultBootstrap::generate_anon_jwt(&self.vault, &project_id, project_ref)
                    .await
                    .map_err(|e| CloudError::Vault(e.to_string()))?
            }
            KeyKind::Service => {
                VaultBootstrap::generate_service_jwt(&self.vault, &project_id, project_ref)
                    .await
                    .map_err(|e| CloudError::Vault(e.to_string()))?
            }
            KeyKind::JwtSigning => {
                return Err(CloudError::InvalidArgument(
                    "cannot create jwt-signing keys via API".to_string(),
                ));
            }
        };

        // Record the key in the database
        let key = self
            .store
            .create_key(&project_id, kind.as_str(), &vault_ref)
            .await?;

        // Audit log
        self.store
            .audit_log(
                Some(&project_id),
                actor,
                AuditAction::KeyCreated,
                serde_json::json!({
                    "key_id": key.id,
                    "kind": kind.as_str(),
                }),
            )
            .await?;

        Ok(KeyResult {
            key,
            value: key_value,
        })
    }

    /// List keys for a project (metadata only, no values).
    pub async fn list(&self, project_ref: &str) -> Result<Vec<ProjectKey>, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        self.store.list_keys(&project.project_id()).await
    }

    /// Rotate a key.
    ///
    /// Creates a new key of the same kind and revokes the old one.
    #[instrument(skip(self))]
    pub async fn rotate(
        &self,
        project_ref: &str,
        key_id: Uuid,
        actor: &str,
    ) -> Result<KeyResult, CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let project_id = project.project_id();

        // Get the existing key
        let old_key = self
            .store
            .get_key(key_id)
            .await?
            .ok_or_else(|| CloudError::KeyNotFound(key_id))?;

        // Verify it belongs to this project
        if old_key.project_id != Uuid::from(project_id) {
            return Err(CloudError::KeyNotFound(key_id));
        }

        // Verify it's not already revoked
        if !old_key.is_active() {
            return Err(CloudError::InvalidArgument("key is already revoked".to_string()));
        }

        let kind: KeyKind = old_key.kind.parse().map_err(|_| {
            CloudError::Internal(format!("invalid key kind: {}", old_key.kind))
        })?;

        // Create new key
        let new_result = self.create(project_ref, kind, actor).await?;

        // Revoke old key
        self.store.revoke_key(key_id).await?;

        // Audit log
        self.store
            .audit_log(
                Some(&project_id),
                actor,
                AuditAction::KeyRotated,
                serde_json::json!({
                    "old_key_id": key_id,
                    "new_key_id": new_result.key.id,
                }),
            )
            .await?;

        Ok(new_result)
    }

    /// Revoke a key.
    #[instrument(skip(self))]
    pub async fn revoke(
        &self,
        project_ref: &str,
        key_id: Uuid,
        actor: &str,
    ) -> Result<(), CloudError> {
        let project = self
            .store
            .get_project_by_ref(project_ref)
            .await?
            .ok_or_else(|| CloudError::ProjectNotFound(project_ref.to_string()))?;

        let project_id = project.project_id();

        // Get the existing key
        let key = self
            .store
            .get_key(key_id)
            .await?
            .ok_or_else(|| CloudError::KeyNotFound(key_id))?;

        // Verify it belongs to this project
        if key.project_id != Uuid::from(project_id) {
            return Err(CloudError::KeyNotFound(key_id));
        }

        // Verify it's not already revoked
        if !key.is_active() {
            return Err(CloudError::InvalidArgument("key is already revoked".to_string()));
        }

        self.store.revoke_key(key_id).await?;

        // Audit log
        self.store
            .audit_log(
                Some(&project_id),
                actor,
                AuditAction::KeyRevoked,
                serde_json::json!({
                    "key_id": key_id,
                }),
            )
            .await?;

        Ok(())
    }
}
