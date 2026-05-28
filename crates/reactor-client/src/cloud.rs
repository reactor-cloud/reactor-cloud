//! Cloud control plane client methods.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ClientResult;
use crate::http::Client;

/// Project information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    #[serde(rename = "ref")]
    pub project_ref: String,
    pub name: String,
    pub owner_user_id: Uuid,
    pub backend_kind: String,
    pub status: String,
    pub region: String,
    pub hostname: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Project creation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectResult {
    pub project: Project,
    pub anon_key: String,
    pub service_key: String,
}

/// Project member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMember {
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Project API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectKey {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Key creation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyResult {
    pub key: ProjectKey,
    pub value: String,
}

/// Audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub project_id: Option<Uuid>,
    pub actor: String,
    pub action: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl Client {
    // =========================================================================
    // Projects
    // =========================================================================

    /// Create a new project.
    pub async fn cloud_projects_create(
        &self,
        name: &str,
        region: Option<&str>,
        owner_user_id: Uuid,
    ) -> ClientResult<CreateProjectResult> {
        let body = serde_json::json!({
            "name": name,
            "region": region,
            "owner_user_id": owner_user_id,
        });

        self.post("/_ops/v1/projects", &body).await
    }

    /// List projects.
    pub async fn cloud_projects_list(
        &self,
        owner: Option<Uuid>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> ClientResult<Vec<Project>> {
        let mut url = "/_ops/v1/projects".to_string();
        let mut params = Vec::new();

        if let Some(owner_id) = owner {
            params.push(format!("owner={}", owner_id));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        self.get(&url).await
    }

    /// Get a project by ref.
    pub async fn cloud_projects_get(&self, project_ref: &str) -> ClientResult<Project> {
        self.get(&format!("/_ops/v1/projects/{}", project_ref))
            .await
    }

    /// Delete a project (soft delete).
    pub async fn cloud_projects_delete(&self, project_ref: &str) -> ClientResult<Project> {
        self.delete(&format!("/_ops/v1/projects/{}", project_ref))
            .await
    }

    // =========================================================================
    // Members
    // =========================================================================

    /// List project members.
    pub async fn cloud_members_list(&self, project_ref: &str) -> ClientResult<Vec<ProjectMember>> {
        self.get(&format!("/_cloud/v1/projects/{}/members", project_ref))
            .await
    }

    /// Add a member to a project.
    pub async fn cloud_members_add(
        &self,
        project_ref: &str,
        user_id: Uuid,
        role: &str,
    ) -> ClientResult<ProjectMember> {
        let body = serde_json::json!({
            "user_id": user_id,
            "role": role,
        });

        self.post(
            &format!("/_cloud/v1/projects/{}/members", project_ref),
            &body,
        )
        .await
    }

    /// Remove a member from a project.
    pub async fn cloud_members_remove(
        &self,
        project_ref: &str,
        user_id: Uuid,
    ) -> ClientResult<serde_json::Value> {
        self.delete(&format!(
            "/_cloud/v1/projects/{}/members/{}",
            project_ref, user_id
        ))
        .await
    }

    // =========================================================================
    // Keys
    // =========================================================================

    /// List project keys.
    pub async fn cloud_keys_list(&self, project_ref: &str) -> ClientResult<Vec<ProjectKey>> {
        self.get(&format!("/_cloud/v1/projects/{}/keys", project_ref))
            .await
    }

    /// Create a new key.
    pub async fn cloud_keys_create(
        &self,
        project_ref: &str,
        kind: &str,
    ) -> ClientResult<CreateKeyResult> {
        let body = serde_json::json!({
            "kind": kind,
        });

        self.post(&format!("/_cloud/v1/projects/{}/keys", project_ref), &body)
            .await
    }

    /// Rotate a key.
    pub async fn cloud_keys_rotate(
        &self,
        project_ref: &str,
        key_id: Uuid,
    ) -> ClientResult<CreateKeyResult> {
        self.post_empty(&format!(
            "/_cloud/v1/projects/{}/keys/{}/rotate",
            project_ref, key_id
        ))
        .await
    }

    /// Revoke a key.
    pub async fn cloud_keys_revoke(
        &self,
        project_ref: &str,
        key_id: Uuid,
    ) -> ClientResult<serde_json::Value> {
        self.delete(&format!(
            "/_cloud/v1/projects/{}/keys/{}",
            project_ref, key_id
        ))
        .await
    }

    // =========================================================================
    // Audit
    // =========================================================================

    /// Get project audit log.
    pub async fn cloud_audit_project(
        &self,
        project_ref: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> ClientResult<Vec<AuditEntry>> {
        let mut url = format!("/_cloud/v1/projects/{}/audit", project_ref);
        let mut params = Vec::new();

        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        self.get(&url).await
    }

    /// Get global audit log.
    pub async fn cloud_audit_global(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> ClientResult<Vec<AuditEntry>> {
        let mut url = "/_cloud/v1/audit".to_string();
        let mut params = Vec::new();

        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        self.get(&url).await
    }
}
