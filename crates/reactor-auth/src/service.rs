//! Authentication service — business logic for auth operations.

use crate::config::AuthConfig;
use crate::crypto::{generate_token_base64url, sha256};
use crate::email::{EmailSender, EmailTemplate};
use crate::password::PasswordHasherService;
use crate::store::{IdentityStore, Invitation, Session};
use crate::token::{
    hash_refresh_token, KeyringManager, RefreshTokenData, TokenIssuer, TokenVerifier,
};
use chrono::{Duration, Utc};
use reactor_core::auth::{AuthError, AuthMethod, Claims, User};
use reactor_core::id::{InvitationId, OrgId, RoleId, SessionId, UserId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Authentication service response for signup/login.
#[derive(Debug)]
pub struct AuthResponse {
    /// The authenticated user.
    pub user: User,
    /// The session.
    pub session: Session,
    /// JWT access token.
    pub access_token: String,
    /// Opaque refresh token.
    pub refresh_token: String,
    /// Access token expiration time.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Response for accepting an invitation.
#[derive(Debug, Serialize, Deserialize)]
pub struct AcceptInvitationResponse {
    /// Status: "joined" or "pending_signup".
    pub status: String,
    /// User ID (if user exists).
    pub user_id: Option<String>,
    /// Organization ID.
    pub org_id: String,
    /// Email address from invitation.
    pub email: String,
    /// Whether signup is required.
    pub requires_signup: bool,
}

/// Authentication service.
pub struct AuthService<S: IdentityStore> {
    store: Arc<S>,
    keyring: Arc<KeyringManager<S>>,
    email_sender: Arc<dyn EmailSender>,
    password_hasher: PasswordHasherService,
    token_issuer: TokenIssuer,
    token_verifier: TokenVerifier,
    config: Arc<AuthConfig>,
}

impl<S: IdentityStore> AuthService<S> {
    /// Create a new auth service.
    pub fn new(
        store: Arc<S>,
        keyring: Arc<KeyringManager<S>>,
        email_sender: Arc<dyn EmailSender>,
        config: Arc<AuthConfig>,
    ) -> Self {
        let token_issuer = TokenIssuer::new(
            config.jwt_issuer.clone(),
            config.jwt_audience.clone(),
            config.access_ttl_secs,
        );
        let token_verifier =
            TokenVerifier::new(config.jwt_issuer.clone(), config.jwt_audience.clone());

        Self {
            store,
            keyring,
            email_sender,
            password_hasher: PasswordHasherService::new(),
            token_issuer,
            token_verifier,
            config,
        }
    }

    /// Sign up a new user with email and password.
    pub async fn signup(
        &self,
        email: &str,
        password: &str,
        metadata: serde_json::Value,
        ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<AuthResponse, AuthError> {
        // Check if user already exists
        if self.store.find_user_by_email(email).await?.is_some() {
            return Err(AuthError::EmailExists);
        }

        // Hash password
        let password_hash = self
            .password_hasher
            .hash(password)
            .map_err(|_| AuthError::Internal)?;

        // Create user
        let user_id = UserId::new();
        let user = self
            .store
            .create_user(user_id, email, Some(&password_hash), metadata)
            .await?;

        // Send verification email (if email sender is enabled)
        if self.email_sender.is_enabled() {
            if let Err(e) = self.send_verification_email(&user).await {
                tracing::warn!(user_id = %user.id, error = %e, "failed to send verification email");
                // Don't fail signup if email fails - user can request resend
            }
        }

        // Create session
        let session_id = SessionId::new();
        let session = self
            .store
            .create_session(
                session_id,
                &user.id,
                vec!["pwd".to_string()],
                ip,
                user_agent,
            )
            .await?;

        // Issue tokens
        self.issue_tokens(&user, &session).await
    }

    /// Send a verification email to a user.
    pub async fn send_verification_email(&self, user: &User) -> Result<(), AuthError> {
        // Generate verification token
        let token = generate_token_base64url(32);
        let token_hash = sha256(token.as_bytes());

        // Token expires in 24 hours
        let expires_at = Utc::now() + Duration::hours(24);

        // Store the token
        self.store
            .create_verification_token(&user.id, &token_hash, "email", expires_at)
            .await?;

        // Build verification link (strip trailing slash from public_url if present)
        let base_url = self.config.public_url.as_str().trim_end_matches('/');
        let verify_link = format!(
            "{}/auth/v1/verify?token={}",
            base_url, token
        );

        // Generate email content
        let (subject, html, text) = EmailTemplate::email_verification(&verify_link, 24);

        // Send email
        self.email_sender
            .send(&user.email, &subject, &html, &text)
            .await?;

        tracing::info!(user_id = %user.id, email = %user.email, "verification email sent");
        Ok(())
    }

    /// Verify a user's email with a verification token.
    pub async fn verify_email(&self, token: &str) -> Result<User, AuthError> {
        let token_hash = sha256(token.as_bytes());

        // Find the token
        let verification = self
            .store
            .find_verification_token_by_hash(&token_hash, "email")
            .await?
            .ok_or(AuthError::InvalidToken)?;

        // Mark token as used
        self.store.use_verification_token(&token_hash).await?;

        // Mark user's email as verified
        let user = self
            .store
            .update_user(
                &verification.user_id,
                None,
                None,
                Some(true), // email_verified = true
                None,
                None,
            )
            .await?;

        tracing::info!(user_id = %user.id, "email verified");
        Ok(user)
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // API Keys
    // ─────────────────────────────────────────────────────────────────────────────

    /// Create an API key for a user.
    ///
    /// Returns the full key (shown once) and the key record.
    pub async fn create_api_key(
        &self,
        user_id: &UserId,
        name: &str,
        scopes: Option<Vec<String>>,
    ) -> Result<(String, crate::store::ApiKey), AuthError> {
        // Generate a random key with a recognizable prefix
        let key_id = reactor_core::ReactorId::new();
        let key_raw = generate_token_base64url(32);
        let full_key = format!("rk_live_{}", key_raw);

        // Store only the hash
        let key_hash = sha256(full_key.as_bytes());

        // Keep prefix for identification (e.g., "rk_live_abc...")
        let prefix = full_key.chars().take(12).collect::<String>();

        let api_key = self
            .store
            .create_api_key(key_id, user_id, name, &key_hash, &prefix, scopes)
            .await?;

        tracing::info!(user_id = %user_id, key_id = %key_id, "API key created");
        Ok((full_key, api_key))
    }

    /// List API keys for a user.
    pub async fn list_api_keys(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<crate::store::ApiKey>, AuthError> {
        self.store.list_user_api_keys(user_id).await
    }

    /// Revoke an API key.
    pub async fn revoke_api_key(
        &self,
        key_id: &reactor_core::ReactorId,
        user_id: &UserId,
    ) -> Result<(), AuthError> {
        self.store.revoke_api_key(key_id, user_id).await?;
        tracing::info!(user_id = %user_id, key_id = %key_id, "API key revoked");
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Password reset
    // ─────────────────────────────────────────────────────────────────────────────

    /// Request a password reset for a user.
    ///
    /// If the user exists and has a verified email, sends a password reset email.
    /// Returns Ok(()) regardless of whether the user exists to prevent enumeration.
    pub async fn request_password_reset(&self, email: &str) -> Result<(), AuthError> {
        // Find user by email
        let user = match self.store.find_user_by_email(email).await? {
            Some(user) => user,
            None => {
                // User doesn't exist - return silently to prevent enumeration
                tracing::debug!(email = %email, "password reset requested for non-existent user");
                return Ok(());
            }
        };

        // Check if user is disabled
        if user.is_disabled() {
            tracing::debug!(user_id = %user.id, "password reset requested for disabled user");
            return Ok(());
        }

        // Check if email is verified (only send reset if verified)
        if !user.email_verified {
            tracing::debug!(user_id = %user.id, "password reset requested for unverified email");
            return Ok(());
        }

        // Generate password reset token
        let token = generate_token_base64url(32);
        let token_hash = sha256(token.as_bytes());

        // Token expires in 1 hour (shorter than email verification for security)
        let expires_at = Utc::now() + Duration::hours(1);

        // Store the token with type "password_reset"
        self.store
            .create_verification_token(&user.id, &token_hash, "password_reset", expires_at)
            .await?;

        // Build reset link
        let base_url = self.config.public_url.as_str().trim_end_matches('/');
        let reset_link = format!("{}/reset?token={}", base_url, token);

        // Generate email content (60 minutes expiry)
        let (subject, html, text) = EmailTemplate::password_reset(&reset_link, 60);

        // Send email
        self.email_sender
            .send(&user.email, &subject, &html, &text)
            .await?;

        tracing::info!(user_id = %user.id, email = %user.email, "password reset email sent");
        Ok(())
    }

    /// Confirm a password reset with the token and new password.
    ///
    /// Validates the token, updates the user's password, marks the token as used,
    /// and optionally revokes all existing sessions.
    pub async fn confirm_password_reset(
        &self,
        token: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        let token_hash = sha256(token.as_bytes());

        // Find the token
        let verification = self
            .store
            .find_verification_token_by_hash(&token_hash, "password_reset")
            .await?
            .ok_or(AuthError::InvalidToken)?;

        // Validate new password using the password policy
        use crate::password::PasswordPolicy;
        let policy = PasswordPolicy::default();
        if let Err(e) = policy.validate(new_password) {
            return Err(AuthError::WeakPassword(e.to_string()));
        }

        // Hash the new password
        let password_hash = self
            .password_hasher
            .hash(new_password)
            .map_err(|_| AuthError::Internal)?;

        // Mark token as used first (prevents reuse even if update fails)
        self.store.use_verification_token(&token_hash).await?;

        // Update user's password
        self.store
            .update_user(
                &verification.user_id,
                None,
                Some(&password_hash),
                None,
                None,
                None,
            )
            .await?;

        // Revoke all existing sessions for security
        self.store
            .revoke_user_sessions(&verification.user_id)
            .await?;

        tracing::info!(user_id = %verification.user_id, "password reset completed, all sessions revoked");
        Ok(())
    }

    /// Find a user by email address.
    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AuthError> {
        self.store.find_user_by_email(email).await
    }

    /// Authenticate with email and password.
    pub async fn password_grant(
        &self,
        email: &str,
        password: &str,
        ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<AuthResponse, AuthError> {
        // Find user
        let user = self
            .store
            .find_user_by_email(email)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        // Check if disabled
        if user.is_disabled() {
            return Err(AuthError::UserDisabled);
        }

        // Verify password
        let password_hash = self
            .store
            .get_password_hash(&user.id)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        let valid = self
            .password_hasher
            .verify(password, &password_hash)
            .map_err(|_| AuthError::Internal)?;

        if !valid {
            return Err(AuthError::InvalidCredentials);
        }

        // Create session
        let session_id = SessionId::new();
        let session = self
            .store
            .create_session(
                session_id,
                &user.id,
                vec!["pwd".to_string()],
                ip,
                user_agent,
            )
            .await?;

        // Issue tokens
        self.issue_tokens(&user, &session).await
    }

    /// Refresh an access token using a refresh token.
    pub async fn refresh(&self, refresh_token: &str) -> Result<AuthResponse, AuthError> {
        let token_hash = hash_refresh_token(refresh_token);

        // Generate new refresh token
        let (new_token, new_data) = RefreshTokenData::new(self.config.refresh_ttl_secs);

        // Rotate in DB (this handles reuse detection)
        let new_refresh = self
            .store
            .rotate_refresh_token(
                &token_hash,
                new_data.id,
                &new_data.token_hash,
                new_data.expires_at,
            )
            .await?;

        // Get session
        let session = self
            .store
            .find_session_by_id(&new_refresh.session_id)
            .await?
            .ok_or(AuthError::SessionRevoked)?;

        // Check if session is revoked
        if session.revoked_at.is_some() {
            return Err(AuthError::SessionRevoked);
        }

        // Get user
        let user = self
            .store
            .find_user_by_id(&session.user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        // Check if disabled
        if user.is_disabled() {
            return Err(AuthError::UserDisabled);
        }

        // Issue new access token
        let keyring = self.keyring.keyring().await;
        let orgs = self.store.list_user_orgs(&user.id).await?;
        let org_ids: Vec<OrgId> = orgs.iter().map(|o| o.id).collect();

        let access_token = self
            .token_issuer
            .issue_access_token(
                &keyring,
                user.id,
                Some(user.email.clone()),
                session.id,
                session.amr.iter().filter_map(|s| parse_amr(s)).collect(),
                org_ids,
                user.default_org_id,
            )
            .map_err(|e| {
                tracing::error!(error = %e, "failed to issue access token");
                AuthError::Internal
            })?;

        let expires_at = Utc::now() + Duration::seconds(self.config.access_ttl_secs as i64);

        Ok(AuthResponse {
            user,
            session,
            access_token,
            refresh_token: new_token,
            expires_at,
        })
    }

    /// Logout — revoke the current session.
    pub async fn logout(&self, session_id: &SessionId) -> Result<(), AuthError> {
        self.store.revoke_session(session_id).await
    }

    /// Verify a token and return the claims.
    pub async fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        let keyring = self.keyring.keyring().await;
        self.token_verifier
            .verify(&keyring, token)
            .map_err(Into::into)
    }

    /// Get a user by ID.
    pub async fn get_user(&self, id: &UserId) -> Result<User, AuthError> {
        self.store
            .find_user_by_id(id)
            .await?
            .ok_or(AuthError::UserNotFound)
    }

    /// Update a user's email, password, or metadata.
    pub async fn update_user(
        &self,
        id: &UserId,
        email: Option<&str>,
        password_hash: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<User, AuthError> {
        // If email is changing, set email_verified to false
        let email_verified = if email.is_some() { Some(false) } else { None };

        self.store
            .update_user(id, email, password_hash, email_verified, metadata, None)
            .await
    }

    /// Soft-delete a user and revoke all their sessions.
    pub async fn delete_user(&self, id: &UserId) -> Result<(), AuthError> {
        // Revoke all sessions first
        self.store.revoke_user_sessions(id).await?;
        // Then disable the user
        self.store.disable_user(id).await
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Organization management
    // ─────────────────────────────────────────────────────────────────────────────

    /// Create an organization and add the creator as owner.
    pub async fn create_org(
        &self,
        creator_id: &UserId,
        name: &str,
        slug: &str,
        metadata: serde_json::Value,
    ) -> Result<crate::store::Org, AuthError> {
        use reactor_core::id::RoleId;

        let org_id = OrgId::new();

        // Create org
        let org = self.store.create_org(org_id, name, slug, metadata).await?;

        // Create system roles with permissions
        let owner_role_id = RoleId::new();
        let admin_role_id = RoleId::new();
        let member_role_id = RoleId::new();

        self.store
            .create_role(
                owner_role_id,
                &org_id,
                "owner",
                Some("Full access"),
                true,
                vec!["*".to_string()],
            )
            .await?;

        self.store
            .create_role(
                admin_role_id,
                &org_id,
                "admin",
                Some("Administrative access"),
                true,
                vec![
                    "auth:*:read".to_string(),
                    "auth:users:write".to_string(),
                    "auth:orgs:read".to_string(),
                    "data:*:*".to_string(),
                    "storage:*:*".to_string(),
                    "functions:*:*".to_string(),
                ],
            )
            .await?;

        self.store
            .create_role(
                member_role_id,
                &org_id,
                "member",
                Some("Basic access"),
                true,
                vec![
                    "auth:users:read".to_string(),
                    "auth:orgs:read".to_string(),
                    "data:*:read".to_string(),
                    "storage:*:read".to_string(),
                ],
            )
            .await?;

        // Add creator as owner
        self.store
            .create_membership(creator_id, &org_id, &owner_role_id)
            .await?;

        Ok(org)
    }

    /// Get an org by ID or slug.
    pub async fn get_org_by_ref(&self, org_ref: &str) -> Result<crate::store::Org, AuthError> {
        // Try parsing as UUID first
        if let Ok(id) = org_ref.parse::<OrgId>() {
            return self
                .store
                .find_org_by_id(&id)
                .await?
                .ok_or(AuthError::OrgNotFound);
        }

        // Otherwise treat as slug
        self.store
            .find_org_by_slug(org_ref)
            .await?
            .ok_or(AuthError::OrgNotFound)
    }

    /// Resolve an org reference (ID or slug) to an OrgId.
    ///
    /// Used by `InProcessAuthClient` to resolve `OrgRef::Slug` values.
    pub async fn resolve_org_ref(&self, org_ref: &str) -> Result<OrgId, AuthError> {
        let org = self.get_org_by_ref(org_ref).await?;
        Ok(org.id)
    }

    /// List orgs a user is a member of.
    pub async fn list_user_orgs(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<crate::store::Org>, AuthError> {
        self.store.list_user_orgs(user_id).await
    }

    /// Check if a user is a member of an org.
    pub async fn check_org_membership(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
    ) -> Result<(), AuthError> {
        let membership = self.store.find_membership(user_id, org_id).await?;
        if membership.is_none() {
            return Err(AuthError::PermissionDenied);
        }
        Ok(())
    }

    /// Require the user to be an owner of the org.
    pub async fn require_org_owner(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
    ) -> Result<(), AuthError> {
        let membership = self
            .store
            .find_membership(user_id, org_id)
            .await?
            .ok_or(AuthError::PermissionDenied)?;

        // Get the role and check if it's 'owner'
        let role = self
            .store
            .find_role_by_id(&membership.role_id)
            .await?
            .ok_or(AuthError::Internal)?;

        if role.name != "owner" {
            return Err(AuthError::PermissionDenied);
        }

        Ok(())
    }

    /// Update an organization.
    pub async fn update_org(
        &self,
        id: &OrgId,
        name: Option<&str>,
        slug: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<crate::store::Org, AuthError> {
        self.store.update_org(id, name, slug, metadata).await
    }

    /// Delete an organization.
    pub async fn delete_org(&self, id: &OrgId) -> Result<(), AuthError> {
        self.store.delete_org(id).await
    }

    /// List roles in an organization with their permissions.
    pub async fn list_org_roles(
        &self,
        org_id: &OrgId,
    ) -> Result<Vec<(crate::store::Role, Vec<String>)>, AuthError> {
        self.store.list_roles_with_permissions(org_id).await
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Membership management
    // ─────────────────────────────────────────────────────────────────────────────

    /// Require the user to be admin or owner of the org.
    pub async fn require_org_admin_or_owner(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
    ) -> Result<(), AuthError> {
        let membership = self
            .store
            .find_membership(user_id, org_id)
            .await?
            .ok_or(AuthError::PermissionDenied)?;

        let role = self
            .store
            .find_role_by_id(&membership.role_id)
            .await?
            .ok_or(AuthError::Internal)?;

        if role.name != "owner" && role.name != "admin" {
            return Err(AuthError::PermissionDenied);
        }

        Ok(())
    }

    /// List members of an organization.
    pub async fn list_org_members(
        &self,
        org_id: &OrgId,
    ) -> Result<Vec<(User, crate::store::Membership, crate::store::Role)>, AuthError> {
        self.store.list_org_members(org_id).await
    }

    /// Get a specific member's details.
    pub async fn get_member(
        &self,
        org_id: &OrgId,
        user_id: &UserId,
    ) -> Result<(User, crate::store::Membership, crate::store::Role), AuthError> {
        let membership = self
            .store
            .find_membership(user_id, org_id)
            .await?
            .ok_or(AuthError::MembershipNotFound)?;

        let user = self
            .store
            .find_user_by_id(user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        let role = self
            .store
            .find_role_by_id(&membership.role_id)
            .await?
            .ok_or(AuthError::Internal)?;

        Ok((user, membership, role))
    }

    /// Update a member's role.
    pub async fn update_member_role(
        &self,
        org_id: &OrgId,
        user_id: &UserId,
        new_role_id: &reactor_core::id::RoleId,
    ) -> Result<(User, crate::store::Membership, crate::store::Role), AuthError> {
        // Get current membership
        let membership = self
            .store
            .find_membership(user_id, org_id)
            .await?
            .ok_or(AuthError::MembershipNotFound)?;

        // Get current role to check if demoting an owner
        let current_role = self
            .store
            .find_role_by_id(&membership.role_id)
            .await?
            .ok_or(AuthError::Internal)?;

        // Verify new role exists and belongs to this org
        let new_role = self
            .store
            .find_role_by_id(new_role_id)
            .await?
            .ok_or(AuthError::RoleNotFound)?;

        if new_role.org_id != *org_id {
            return Err(AuthError::RoleNotFound);
        }

        // If demoting from owner, check for last owner
        if current_role.name == "owner" && new_role.name != "owner" {
            let owner_count = self
                .store
                .count_members_with_role(org_id, &membership.role_id)
                .await?;
            if owner_count <= 1 {
                return Err(AuthError::LastOwner);
            }
        }

        // Update the role
        let updated_membership = self
            .store
            .update_membership_role(user_id, org_id, new_role_id)
            .await?;

        let user = self
            .store
            .find_user_by_id(user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        Ok((user, updated_membership, new_role))
    }

    /// Remove a member from an organization.
    pub async fn remove_member(&self, org_id: &OrgId, user_id: &UserId) -> Result<(), AuthError> {
        // Get current membership
        let membership = self
            .store
            .find_membership(user_id, org_id)
            .await?
            .ok_or(AuthError::MembershipNotFound)?;

        // Get current role to check if removing an owner
        let role = self
            .store
            .find_role_by_id(&membership.role_id)
            .await?
            .ok_or(AuthError::Internal)?;

        // If removing an owner, check for last owner
        if role.name == "owner" {
            let owner_count = self
                .store
                .count_members_with_role(org_id, &membership.role_id)
                .await?;
            if owner_count <= 1 {
                return Err(AuthError::LastOwner);
            }
        }

        // Remove the membership
        self.store.delete_membership(user_id, org_id).await
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Invitations
    // ─────────────────────────────────────────────────────────────────────────────

    /// Create an invitation.
    pub async fn create_invitation(
        &self,
        org_id: &OrgId,
        email: &str,
        role_id: &RoleId,
    ) -> Result<(Invitation, String), AuthError> {
        // Verify role exists and belongs to this org
        let role = self
            .store
            .find_role_by_id(role_id)
            .await?
            .ok_or(AuthError::RoleNotFound)?;

        if role.org_id != *org_id {
            return Err(AuthError::RoleNotFound);
        }

        // Get org for email template
        let org = self
            .store
            .find_org_by_id(org_id)
            .await?
            .ok_or(AuthError::OrgNotFound)?;

        // Generate invitation token
        let token = generate_token_base64url(32);
        let token_hash = sha256(token.as_bytes());

        // Invitation expires in 7 days
        let expires_at = Utc::now() + Duration::days(7);

        self.store
            .create_invitation(&token_hash, email, org_id, role_id, expires_at)
            .await?;

        // Generate invitation link
        let invite_link = format!(
            "{}/auth/v1/invitations/accept?token={}",
            self.config.public_url, token
        );

        // Get invitation back
        let invitation = self
            .store
            .find_invitation_by_hash(&token_hash)
            .await?
            .ok_or(AuthError::Internal)?;

        // Send email (if SMTP is configured)
        if self.email_sender.is_enabled() {
            let (subject, html, text) = EmailTemplate::invitation(
                &org.name,
                &role.name,
                &invite_link,
                168, // 7 days in hours
            );

            if let Err(e) = self.email_sender.send(email, &subject, &html, &text).await {
                tracing::warn!(error = %e, "failed to send invitation email, but invitation link is still valid");
            }
        }

        Ok((invitation, invite_link))
    }

    /// List pending invitations for an organization.
    pub async fn list_org_invitations(&self, org_id: &OrgId) -> Result<Vec<Invitation>, AuthError> {
        self.store.list_org_invitations(org_id).await
    }

    /// Delete an invitation by ID.
    ///
    /// Note: This is a placeholder - actual implementation needs a delete_by_id store method.
    pub async fn delete_invitation(&self, _invitation_id: &InvitationId) -> Result<(), AuthError> {
        // TODO: Add delete_invitation_by_id to IdentityStore trait
        // For now invitations can only be deleted by admins listing them
        Err(AuthError::InvitationNotFound)
    }

    /// Accept an invitation.
    pub async fn accept_invitation(
        &self,
        token: &str,
    ) -> Result<AcceptInvitationResponse, AuthError> {
        let token_hash = sha256(token.as_bytes());

        let invitation = self
            .store
            .find_invitation_by_hash(&token_hash)
            .await?
            .ok_or(AuthError::InvitationNotFound)?;

        // Check expiration
        if invitation.expires_at < Utc::now() {
            return Err(AuthError::InvitationNotFound);
        }

        // Check if user exists with this email
        let existing_user = self.store.find_user_by_email(&invitation.email).await?;

        if let Some(user) = existing_user {
            // User exists - create membership directly
            self.store
                .create_membership(&user.id, &invitation.org_id, &invitation.role_id)
                .await?;

            // Mark invitation as used
            self.store.use_invitation(&token_hash).await?;

            Ok(AcceptInvitationResponse {
                status: "joined".to_string(),
                user_id: Some(user.id.to_string()),
                org_id: invitation.org_id.to_string(),
                email: invitation.email,
                requires_signup: false,
            })
        } else {
            // User doesn't exist - return info for signup flow
            // Don't mark as used yet - that happens after successful signup
            Ok(AcceptInvitationResponse {
                status: "pending_signup".to_string(),
                user_id: None,
                org_id: invitation.org_id.to_string(),
                email: invitation.email,
                requires_signup: true,
            })
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Permissions
    // ─────────────────────────────────────────────────────────────────────────────

    /// Get user's effective permissions in an organization.
    pub async fn get_user_permissions(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
    ) -> Result<Vec<String>, AuthError> {
        self.store.get_user_permissions(user_id, org_id).await
    }

    /// Get the current keyring (for JWKS).
    pub async fn keyring(&self) -> crate::token::Keyring {
        let guard = self.keyring.keyring().await;
        (*guard).clone()
    }

    /// Issue tokens for a user/session pair.
    async fn issue_tokens(
        &self,
        user: &User,
        session: &Session,
    ) -> Result<AuthResponse, AuthError> {
        // Get user's orgs
        let orgs = self.store.list_user_orgs(&user.id).await?;
        let org_ids: Vec<OrgId> = orgs.iter().map(|o| o.id).collect();

        // Issue access token
        let keyring = self.keyring.keyring().await;
        let access_token = self
            .token_issuer
            .issue_access_token(
                &keyring,
                user.id,
                Some(user.email.clone()),
                session.id,
                session.amr.iter().filter_map(|s| parse_amr(s)).collect(),
                org_ids,
                user.default_org_id,
            )
            .map_err(|e| {
                tracing::error!(error = %e, "failed to issue access token");
                AuthError::Internal
            })?;

        // Issue refresh token
        let (refresh_token, refresh_data) = RefreshTokenData::new(self.config.refresh_ttl_secs);
        self.store
            .create_refresh_token(
                refresh_data.id,
                &session.id,
                &refresh_data.token_hash,
                refresh_data.expires_at,
            )
            .await?;

        let expires_at = Utc::now() + Duration::seconds(self.config.access_ttl_secs as i64);

        Ok(AuthResponse {
            user: user.clone(),
            session: session.clone(),
            access_token,
            refresh_token,
            expires_at,
        })
    }

    /// Issue tokens with MFA timestamp after successful WebAuthn authentication.
    ///
    /// This is called when a user completes WebAuthn step-up authentication.
    /// The new token will have `mfa_at` set to the current timestamp.
    pub async fn issue_mfa_tokens(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
        scopes: Vec<String>,
    ) -> Result<MfaAuthResponse, AuthError> {
        // Get user
        let user = self.store.find_user_by_id(user_id).await?
            .ok_or(AuthError::UserNotFound)?;

        // Get session
        let session = self.store.find_session_by_id(session_id).await?
            .ok_or(AuthError::InvalidToken)?;

        // AMR with hardware key indicator
        let mut amr = session.amr.clone();
        if !amr.contains(&"hwk".to_string()) {
            amr.push("hwk".to_string());
        }

        // Get user's orgs
        let orgs = self.store.list_user_orgs(&user.id).await?;
        let org_ids: Vec<OrgId> = orgs.iter().map(|o| o.id).collect();

        // Issue access token with mfa_at
        let mfa_at = Utc::now().timestamp();
        let keyring = self.keyring.keyring().await;
        let access_token = self
            .token_issuer
            .issue_access_token_with_scopes(
                &keyring,
                user.id,
                Some(user.email.clone()),
                session.id,
                amr.iter().filter_map(|s| parse_amr(s)).collect(),
                org_ids,
                user.default_org_id,
                scopes.clone(),
                Some(mfa_at),
            )
            .map_err(|e| {
                tracing::error!(error = %e, "failed to issue MFA access token");
                AuthError::Internal
            })?;

        // Issue refresh token
        let (refresh_token, refresh_data) = RefreshTokenData::new(self.config.refresh_ttl_secs);
        self.store
            .create_refresh_token(
                refresh_data.id,
                &session.id,
                &refresh_data.token_hash,
                refresh_data.expires_at,
            )
            .await?;

        let expires_at = Utc::now() + Duration::seconds(self.config.access_ttl_secs as i64);

        Ok(MfaAuthResponse {
            access_token,
            refresh_token,
            expires_at,
            mfa_at,
            scopes,
        })
    }
}

/// Response for MFA token issuance.
#[derive(Debug)]
pub struct MfaAuthResponse {
    /// JWT access token with mfa_at set.
    pub access_token: String,
    /// Opaque refresh token.
    pub refresh_token: String,
    /// Access token expiration time.
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// MFA timestamp (Unix epoch seconds).
    pub mfa_at: i64,
    /// Scopes granted.
    pub scopes: Vec<String>,
}

fn parse_amr(s: &str) -> Option<AuthMethod> {
    match s {
        "pwd" => Some(AuthMethod::Pwd),
        "totp" => Some(AuthMethod::Totp),
        "apikey" => Some(AuthMethod::Apikey),
        "magic_link" => Some(AuthMethod::MagicLink),
        _ => None,
    }
}

/// Response for authorization code exchange.
#[derive(Debug)]
pub struct AuthorizationCodeResponse {
    /// The authenticated user.
    pub user: User,
    /// The session.
    pub session: Session,
    /// JWT access token.
    pub access_token: String,
    /// Opaque refresh token.
    pub refresh_token: String,
    /// Access token expiration time.
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Scopes granted.
    pub scopes: Vec<String>,
}

impl<S: IdentityStore> AuthService<S> {
    // ─────────────────────────────────────────────────────────────────────────────
    // Authorization Codes (PKCE)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Create an authorization code for PKCE flow.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_authorization_code(
        &self,
        user_id: UserId,
        client_id: &str,
        redirect_uri: &str,
        scopes: Vec<String>,
        code_challenge: &str,
        code_challenge_method: &str,
        nonce: Option<&str>,
        state: Option<&str>,
        session_id: Option<SessionId>,
    ) -> Result<String, AuthError> {
        // Generate the code (random 32 bytes, base64url encoded)
        let code = generate_token_base64url(32);
        let code_hash = sha256(code.as_bytes());

        // Code expires in 10 minutes
        let expires_at = Utc::now() + Duration::minutes(10);

        let code_id = reactor_core::ReactorId::new();
        self.store
            .create_authorization_code(
                code_id,
                &user_id,
                client_id,
                redirect_uri,
                scopes,
                &code_hash,
                code_challenge,
                code_challenge_method,
                nonce,
                state,
                expires_at,
                session_id.as_ref(),
            )
            .await?;

        tracing::info!(user_id = %user_id, client_id = %client_id, "authorization code created");
        Ok(code)
    }

    /// Exchange an authorization code for tokens (PKCE).
    pub async fn exchange_authorization_code(
        &self,
        code: &str,
        code_verifier: &str,
        client_id: &str,
        redirect_uri: &str,
        ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<AuthorizationCodeResponse, AuthError> {
        let code_hash = sha256(code.as_bytes());

        // Find the authorization code
        let auth_code = self
            .store
            .find_authorization_code_by_hash(&code_hash)
            .await?
            .ok_or(AuthError::InvalidToken)?;

        // Validate client_id and redirect_uri
        if auth_code.client_id != client_id {
            tracing::warn!(
                expected_client = %auth_code.client_id,
                actual_client = %client_id,
                "client_id mismatch in code exchange"
            );
            return Err(AuthError::InvalidToken);
        }
        if auth_code.redirect_uri != redirect_uri {
            tracing::warn!(
                expected_redirect = %auth_code.redirect_uri,
                actual_redirect = %redirect_uri,
                "redirect_uri mismatch in code exchange"
            );
            return Err(AuthError::InvalidToken);
        }

        // Validate PKCE code_verifier
        if !verify_pkce_challenge(code_verifier, &auth_code.code_challenge, &auth_code.code_challenge_method) {
            tracing::warn!("PKCE code_verifier validation failed");
            return Err(AuthError::InvalidToken);
        }

        // Get the user
        let user = self
            .store
            .find_user_by_id(&auth_code.user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        // Check if disabled
        if user.is_disabled() {
            return Err(AuthError::UserDisabled);
        }

        // Create a new session
        let session_id = SessionId::new();
        let session = self
            .store
            .create_session(
                session_id,
                &user.id,
                vec!["pkce".to_string()],
                ip,
                user_agent,
            )
            .await?;

        // Mark the code as used
        self.store
            .use_authorization_code(&code_hash, &session.id)
            .await?;

        // Issue tokens
        let auth_response = self.issue_tokens(&user, &session).await?;

        Ok(AuthorizationCodeResponse {
            user: auth_response.user,
            session: auth_response.session,
            access_token: auth_response.access_token,
            refresh_token: auth_response.refresh_token,
            expires_at: auth_response.expires_at,
            scopes: auth_code.scopes,
        })
    }
}

impl<S: IdentityStore> AuthService<S> {
    // ─────────────────────────────────────────────────────────────────────────────
    // Platform Operators
    // ─────────────────────────────────────────────────────────────────────────────

    /// Promote a user to platform operator.
    ///
    /// This grants the `platform_operator` role to the specified user.
    pub async fn promote_to_platform_operator(
        &self,
        user_email: &str,
        granted_by: Option<UserId>,
    ) -> Result<UserId, AuthError> {
        // Find the user by email
        let user = self
            .store
            .find_user_by_email(user_email)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        // Grant the platform_operator role
        self.store
            .grant_platform_role(&user.id, "platform_operator", granted_by.as_ref())
            .await?;

        tracing::info!(
            user_id = %user.id,
            email = %user_email,
            granted_by = ?granted_by,
            "user promoted to platform_operator"
        );

        Ok(user.id)
    }

    /// Check if any platform operators exist.
    ///
    /// This is used to determine if bootstrap is needed.
    pub async fn platform_operators_exist(&self) -> Result<bool, AuthError> {
        self.store.platform_operators_exist().await
    }

    /// Get platform permissions for a user.
    ///
    /// Returns a list of (permission, requires_step_up) tuples.
    pub async fn get_platform_permissions(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<(String, bool)>, AuthError> {
        self.store.get_platform_permissions(user_id).await
    }

    /// Check if a user has a specific platform permission.
    ///
    /// Returns (has_permission, requires_step_up).
    pub async fn check_platform_permission(
        &self,
        user_id: &UserId,
        required_permission: &str,
    ) -> Result<(bool, bool), AuthError> {
        let permissions = self.get_platform_permissions(user_id).await?;

        for (perm, requires_step_up) in permissions {
            if matches_permission(&perm, required_permission) {
                return Ok((true, requires_step_up));
            }
        }

        Ok((false, false))
    }
}

/// Check if a permission pattern matches a required permission.
fn matches_permission(pattern: &str, required: &str) -> bool {
    // Handle wildcard patterns
    if pattern == "*" {
        return true;
    }
    if pattern.ends_with(":*") {
        let prefix = &pattern[..pattern.len() - 1]; // Remove the "*"
        return required.starts_with(prefix);
    }
    pattern == required
}

/// Verify PKCE code_challenge against code_verifier.
fn verify_pkce_challenge(verifier: &str, challenge: &str, method: &str) -> bool {
    match method {
        "S256" => {
            // SHA256 hash of verifier, then base64url encode
            use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
            use sha2::{Sha256, Digest};

            let mut hasher = Sha256::new();
            hasher.update(verifier.as_bytes());
            let hash = hasher.finalize();
            let expected = URL_SAFE_NO_PAD.encode(hash);
            expected == challenge
        }
        "plain" => verifier == challenge,
        _ => false,
    }
}
