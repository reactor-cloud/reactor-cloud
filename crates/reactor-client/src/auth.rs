//! Auth capability client (`/auth/v1/*`).

use crate::error::ClientResult;
use crate::http::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current user information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    pub user_id: Uuid,
    pub email: String,
    #[serde(default)]
    pub orgs: Vec<OrgMembership>,
}

/// Organization membership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgMembership {
    pub org_id: Uuid,
    pub org_slug: String,
    pub role: String,
}

/// Organization details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Organization member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub user_id: Uuid,
    #[serde(default)]
    pub email: Option<String>,
    pub role: String,
    pub joined_at: chrono::DateTime<chrono::Utc>,
}

/// API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Newly created API key with secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyWithSecret {
    pub id: Uuid,
    pub name: String,
    pub secret: String,
}

/// Invitation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub status: InvitationStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Invitation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Expired,
    Revoked,
}

impl Client {
    /// Get current user information.
    pub async fn auth_me(&self) -> ClientResult<CurrentUser> {
        self.get("/auth/v1/me").await
    }

    // ========================================================================
    // Organizations
    // ========================================================================

    /// List organizations.
    pub async fn auth_orgs_list(&self) -> ClientResult<Vec<Org>> {
        self.get("/auth/v1/_admin/orgs").await
    }

    /// Get organization by ID.
    pub async fn auth_org_get(&self, org_id: Uuid) -> ClientResult<Org> {
        self.get(&format!("/auth/v1/_admin/orgs/{}", org_id)).await
    }

    /// Create an organization.
    pub async fn auth_org_create(&self, name: &str, slug: &str) -> ClientResult<Org> {
        #[derive(Serialize)]
        struct CreateOrg<'a> {
            name: &'a str,
            slug: &'a str,
        }
        // The auth router exposes the org-CRUD endpoints under `/auth/v1/orgs`
        // (user-scoped — the caller becomes the owner). The legacy
        // `/auth/v1/_admin/orgs` path was never wired up server-side.
        self.post("/auth/v1/orgs", &CreateOrg { name, slug })
            .await
    }

    /// Update an organization.
    pub async fn auth_org_update(
        &self,
        org_id: Uuid,
        name: Option<&str>,
        slug: Option<&str>,
    ) -> ClientResult<Org> {
        #[derive(Serialize)]
        struct UpdateOrg<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            name: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            slug: Option<&'a str>,
        }
        self.patch(
            &format!("/auth/v1/_admin/orgs/{}", org_id),
            &UpdateOrg { name, slug },
        )
        .await
    }

    /// Delete an organization.
    pub async fn auth_org_delete(&self, org_id: Uuid) -> ClientResult<()> {
        self.delete::<serde_json::Value>(&format!("/auth/v1/_admin/orgs/{}", org_id))
            .await?;
        Ok(())
    }

    // ========================================================================
    // Members
    // ========================================================================

    /// List organization members.
    pub async fn auth_members_list(&self, org_id: Uuid) -> ClientResult<Vec<Member>> {
        self.get(&format!("/auth/v1/_admin/orgs/{}/members", org_id))
            .await
    }

    /// Add a member to an organization.
    pub async fn auth_member_add(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
    ) -> ClientResult<Member> {
        #[derive(Serialize)]
        struct AddMember<'a> {
            user_id: Uuid,
            role: &'a str,
        }
        self.post(
            &format!("/auth/v1/_admin/orgs/{}/members", org_id),
            &AddMember { user_id, role },
        )
        .await
    }

    /// Update a member's role.
    pub async fn auth_member_update(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
    ) -> ClientResult<Member> {
        #[derive(Serialize)]
        struct UpdateMember<'a> {
            role: &'a str,
        }
        self.patch(
            &format!("/auth/v1/_admin/orgs/{}/members/{}", org_id, user_id),
            &UpdateMember { role },
        )
        .await
    }

    /// Remove a member from an organization.
    pub async fn auth_member_remove(&self, org_id: Uuid, user_id: Uuid) -> ClientResult<()> {
        self.delete::<serde_json::Value>(&format!(
            "/auth/v1/_admin/orgs/{}/members/{}",
            org_id, user_id
        ))
        .await?;
        Ok(())
    }

    // ========================================================================
    // API Keys
    // ========================================================================

    /// List API keys.
    pub async fn auth_keys_list(&self, org_id: Uuid) -> ClientResult<Vec<ApiKey>> {
        self.get(&format!("/auth/v1/_admin/orgs/{}/keys", org_id))
            .await
    }

    /// Create an API key.
    pub async fn auth_key_create(&self, org_id: Uuid, name: &str) -> ClientResult<ApiKeyWithSecret> {
        #[derive(Serialize)]
        struct CreateKey<'a> {
            name: &'a str,
        }
        self.post(
            &format!("/auth/v1/_admin/orgs/{}/keys", org_id),
            &CreateKey { name },
        )
        .await
    }

    /// Revoke an API key.
    pub async fn auth_key_revoke(&self, key_id: Uuid) -> ClientResult<()> {
        self.delete::<serde_json::Value>(&format!("/auth/v1/_admin/keys/{}", key_id))
            .await?;
        Ok(())
    }

    // ========================================================================
    // Invitations
    // ========================================================================

    /// List invitations.
    pub async fn auth_invitations_list(&self, org_id: Uuid) -> ClientResult<Vec<Invitation>> {
        self.get(&format!("/auth/v1/_admin/orgs/{}/invitations", org_id))
            .await
    }

    /// Create an invitation.
    pub async fn auth_invitation_create(
        &self,
        org_id: Uuid,
        email: &str,
        role: &str,
    ) -> ClientResult<Invitation> {
        #[derive(Serialize)]
        struct CreateInvitation<'a> {
            email: &'a str,
            role: &'a str,
        }
        self.post(
            &format!("/auth/v1/_admin/orgs/{}/invitations", org_id),
            &CreateInvitation { email, role },
        )
        .await
    }

    /// Revoke an invitation.
    pub async fn auth_invitation_revoke(&self, invitation_id: Uuid) -> ClientResult<()> {
        self.delete::<serde_json::Value>(&format!(
            "/auth/v1/_admin/invitations/{}",
            invitation_id
        ))
        .await?;
        Ok(())
    }
}
