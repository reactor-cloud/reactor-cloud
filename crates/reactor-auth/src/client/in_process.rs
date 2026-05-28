//! In-process AuthClient implementation.
//!
//! This client calls the AuthService directly without HTTP overhead.
//! Used when reactor-auth is embedded in the same process as other capabilities.

use crate::service::AuthService;
use crate::store::IdentityStore;
use async_trait::async_trait;
use reactor_core::auth::{AuthClient, AuthCtx, AuthError, Claims, Jwks, OrgRef, User};
use reactor_core::id::UserId;
use std::sync::Arc;

/// In-process AuthClient that calls AuthService directly.
pub struct InProcessAuthClient<S: IdentityStore> {
    service: Arc<AuthService<S>>,
}

impl<S: IdentityStore> InProcessAuthClient<S> {
    /// Create a new in-process auth client.
    pub fn new(service: Arc<AuthService<S>>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl<S: IdentityStore> AuthClient for InProcessAuthClient<S> {
    async fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        self.service.verify_token(token).await
    }

    async fn resolve_ctx(
        &self,
        token: &str,
        requested_org: Option<&OrgRef>,
    ) -> Result<AuthCtx, AuthError> {
        let claims = self.service.verify_token(token).await?;

        // Resolve OrgRef to OrgId (slugs require DB lookup)
        let resolved_org = match requested_org {
            Some(OrgRef::Id(id)) => Some(*id),
            Some(OrgRef::Slug(slug)) => {
                let org = self.service.resolve_org_ref(slug).await?;
                Some(org)
            }
            None => None,
        };

        // Determine active org: explicit > default > None
        let active_org = resolved_org.or(claims.default_org);

        // Verify membership if org is specified
        if let Some(ref oid) = active_org {
            if !claims.orgs.contains(oid) {
                return Err(AuthError::NotOrgMember);
            }
        }

        // Get permissions if we have an org context
        let permissions = if let Some(ref oid) = active_org {
            if let Some(uid) = claims.user_id() {
                self.service.get_user_permissions(&uid, oid).await?
            } else {
                // API key - no permissions yet
                vec![]
            }
        } else {
            vec![]
        };

        Ok(AuthCtx {
            claims,
            active_org,
            permissions,
        })
    }

    async fn get_user(&self, user_id: &UserId) -> Result<User, AuthError> {
        self.service.get_user(user_id).await
    }

    async fn check_permission(&self, ctx: &AuthCtx, permission: &str) -> Result<bool, AuthError> {
        Ok(ctx.has_permission(permission))
    }

    async fn jwks(&self) -> Result<Jwks, AuthError> {
        let keyring = self.service.keyring().await;
        keyring.to_jwks().map_err(|e| {
            tracing::error!(error = %e, "failed to convert keyring to JWKS");
            AuthError::Internal
        })
    }
}
