//! Identity store trait and implementations.

mod postgres;

pub use postgres::PgIdentityStore;

/// Get the SQLx migrator for reactor-auth migrations.
///
/// Use this to apply migrations at startup:
/// ```ignore
/// reactor_auth::store::migrator().run(&pool).await?;
/// ```
pub fn migrator() -> sqlx::migrate::Migrator {
    sqlx::migrate!("./migrations")
}

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reactor_core::auth::{AuthError, User};
use reactor_core::id::{InvitationId, OrgId, RoleId, SessionId, UserId};

/// A session record.
#[derive(Debug, Clone)]
pub struct Session {
    /// Session ID.
    pub id: SessionId,
    /// User ID.
    pub user_id: UserId,
    /// Authentication methods used.
    pub amr: Vec<String>,
    /// Client IP address.
    pub ip: Option<String>,
    /// User agent string.
    pub user_agent: Option<String>,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last seen.
    pub last_seen_at: DateTime<Utc>,
    /// When the session was revoked (if revoked).
    pub revoked_at: Option<DateTime<Utc>>,
}

/// A refresh token record.
#[derive(Debug, Clone)]
pub struct RefreshToken {
    /// Token ID.
    pub id: reactor_core::ReactorId,
    /// Session ID.
    pub session_id: SessionId,
    /// Token hash (for lookup).
    pub token_hash: Vec<u8>,
    /// When the token was issued.
    pub issued_at: DateTime<Utc>,
    /// When the token expires.
    pub expires_at: DateTime<Utc>,
    /// When the token was used (if used).
    pub used_at: Option<DateTime<Utc>>,
    /// ID of the replacement token (if rotated).
    pub replaced_by: Option<reactor_core::ReactorId>,
}

/// An organization record.
#[derive(Debug, Clone)]
pub struct Org {
    /// Organization ID.
    pub id: OrgId,
    /// URL-safe slug.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// Custom metadata.
    pub metadata: serde_json::Value,
    /// When the org was created.
    pub created_at: DateTime<Utc>,
    /// When the org was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A role record.
#[derive(Debug, Clone)]
pub struct Role {
    /// Role ID.
    pub id: RoleId,
    /// Organization ID.
    pub org_id: OrgId,
    /// Role name.
    pub name: String,
    /// Role description.
    pub description: Option<String>,
    /// Whether this is a system role.
    pub is_system: bool,
    /// When the role was created.
    pub created_at: DateTime<Utc>,
}

/// A membership record.
#[derive(Debug, Clone)]
pub struct Membership {
    /// User ID.
    pub user_id: UserId,
    /// Organization ID.
    pub org_id: OrgId,
    /// Role ID.
    pub role_id: RoleId,
    /// When the user joined.
    pub joined_at: DateTime<Utc>,
}

/// An invitation record.
#[derive(Debug, Clone)]
pub struct Invitation {
    /// Invitation ID (derived from token hash).
    pub id: InvitationId,
    /// Target email address.
    pub email: String,
    /// Organization ID.
    pub org_id: OrgId,
    /// Role to assign on acceptance.
    pub role_id: RoleId,
    /// When the invitation was created.
    pub created_at: DateTime<Utc>,
    /// When the invitation expires.
    pub expires_at: DateTime<Utc>,
}

/// A verification token record (for email verification, password reset, etc.).
#[derive(Debug, Clone)]
pub struct VerificationToken {
    /// Token ID.
    pub id: reactor_core::ReactorId,
    /// User ID.
    pub user_id: UserId,
    /// Token type (e.g., "email", "password_reset").
    pub token_type: String,
    /// When the token was created.
    pub created_at: DateTime<Utc>,
    /// When the token expires.
    pub expires_at: DateTime<Utc>,
    /// When the token was used (if used).
    pub used_at: Option<DateTime<Utc>>,
}

/// An authorization code record (for PKCE OAuth flow).
#[derive(Debug, Clone)]
pub struct AuthorizationCode {
    /// Code ID.
    pub id: reactor_core::ReactorId,
    /// User ID.
    pub user_id: UserId,
    /// Client ID (e.g., "reactor-cli").
    pub client_id: String,
    /// Redirect URI.
    pub redirect_uri: String,
    /// Requested scopes.
    pub scopes: Vec<String>,
    /// PKCE code challenge (S256).
    pub code_challenge: String,
    /// PKCE code challenge method.
    pub code_challenge_method: String,
    /// Optional nonce for OIDC.
    pub nonce: Option<String>,
    /// State parameter.
    pub state: Option<String>,
    /// When the code was created.
    pub created_at: DateTime<Utc>,
    /// When the code expires.
    pub expires_at: DateTime<Utc>,
    /// When the code was used (if used).
    pub used_at: Option<DateTime<Utc>>,
    /// Session ID created from this code.
    pub session_id: Option<SessionId>,
}

/// An audit event record.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Event ID.
    pub id: reactor_core::ReactorId,
    /// Timestamp.
    pub ts: chrono::DateTime<chrono::Utc>,
    /// Actor user ID (if user).
    pub actor_user_id: Option<UserId>,
    /// Actor API key ID (if API key).
    pub actor_apikey_id: Option<reactor_core::ReactorId>,
    /// Organization context.
    pub org_id: Option<OrgId>,
    /// Event type (e.g., "user.signup", "session.created").
    pub event_type: String,
    /// Resource identifier (if applicable).
    pub resource: Option<String>,
    /// Client IP address.
    pub ip: Option<String>,
    /// User agent string.
    pub user_agent: Option<String>,
    /// Additional event details.
    pub details: serde_json::Value,
}

impl AuditEvent {
    /// Create a new audit event.
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            id: reactor_core::ReactorId::new(),
            ts: chrono::Utc::now(),
            actor_user_id: None,
            actor_apikey_id: None,
            org_id: None,
            event_type: event_type.into(),
            resource: None,
            ip: None,
            user_agent: None,
            details: serde_json::json!({}),
        }
    }

    /// Set the actor user ID.
    pub fn with_user(mut self, user_id: UserId) -> Self {
        self.actor_user_id = Some(user_id);
        self
    }

    /// Set the org context.
    pub fn with_org(mut self, org_id: OrgId) -> Self {
        self.org_id = Some(org_id);
        self
    }

    /// Set the client info.
    pub fn with_client(mut self, ip: Option<&str>, user_agent: Option<&str>) -> Self {
        self.ip = ip.map(|s| s.to_string());
        self.user_agent = user_agent.map(|s| s.to_string());
        self
    }

    /// Set the resource.
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    /// Set additional details.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }
}

/// An API key record.
#[derive(Debug, Clone)]
pub struct ApiKey {
    /// API key ID.
    pub id: reactor_core::ReactorId,
    /// User ID.
    pub user_id: UserId,
    /// Key name (user-provided label).
    pub name: String,
    /// Key prefix (first 8 chars of the full key, for identification).
    pub prefix: String,
    /// Optional scopes (null = all permissions).
    pub scopes: Option<Vec<String>>,
    /// When the key was created.
    pub created_at: DateTime<Utc>,
    /// When the key was last used.
    pub last_used_at: Option<DateTime<Utc>>,
    /// When the key was revoked (if revoked).
    pub revoked_at: Option<DateTime<Utc>>,
}

/// A signing key record.
#[derive(Debug, Clone)]
pub struct SigningKey {
    /// Key ID.
    pub kid: String,
    /// Algorithm (e.g., "RS256").
    pub algorithm: String,
    /// Private key PEM (encrypted).
    pub private_key_pem: String,
    /// Public key PEM.
    pub public_key_pem: String,
    /// When the key was created.
    pub created_at: DateTime<Utc>,
    /// When the key was activated.
    pub activated_at: DateTime<Utc>,
    /// When the key was rotated out.
    pub rotated_at: Option<DateTime<Utc>>,
    /// When the key was retired.
    pub retired_at: Option<DateTime<Utc>>,
}

/// Storage trait for identity data.
///
/// This trait abstracts the persistence layer, allowing different
/// implementations (Postgres, SQLite in the future).
#[async_trait]
pub trait IdentityStore: Send + Sync + 'static {
    // ========== Users ==========

    /// Find a user by ID.
    async fn find_user_by_id(&self, id: &UserId) -> Result<Option<User>, AuthError>;

    /// Find a user by email.
    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AuthError>;

    /// Create a new user.
    async fn create_user(
        &self,
        id: UserId,
        email: &str,
        password_hash: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<User, AuthError>;

    /// Update a user.
    async fn update_user(
        &self,
        id: &UserId,
        email: Option<&str>,
        password_hash: Option<&str>,
        email_verified: Option<bool>,
        metadata: Option<serde_json::Value>,
        default_org_id: Option<Option<OrgId>>,
    ) -> Result<User, AuthError>;

    /// Get a user's password hash.
    async fn get_password_hash(&self, user_id: &UserId) -> Result<Option<String>, AuthError>;

    /// Soft delete a user.
    async fn disable_user(&self, id: &UserId) -> Result<(), AuthError>;

    // ========== Sessions ==========

    /// Create a new session.
    async fn create_session(
        &self,
        id: SessionId,
        user_id: &UserId,
        amr: Vec<String>,
        ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<Session, AuthError>;

    /// Find a session by ID.
    async fn find_session_by_id(&self, id: &SessionId) -> Result<Option<Session>, AuthError>;

    /// Revoke a session.
    async fn revoke_session(&self, id: &SessionId) -> Result<(), AuthError>;

    /// Revoke all sessions for a user.
    async fn revoke_user_sessions(&self, user_id: &UserId) -> Result<u64, AuthError>;

    // ========== Refresh Tokens ==========

    /// Create a refresh token.
    async fn create_refresh_token(
        &self,
        id: reactor_core::ReactorId,
        session_id: &SessionId,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<RefreshToken, AuthError>;

    /// Find a refresh token by hash.
    async fn find_refresh_token_by_hash(
        &self,
        token_hash: &[u8],
    ) -> Result<Option<RefreshToken>, AuthError>;

    /// Mark a refresh token as used and create a replacement.
    async fn rotate_refresh_token(
        &self,
        old_token_hash: &[u8],
        new_id: reactor_core::ReactorId,
        new_token_hash: &[u8],
        new_expires_at: DateTime<Utc>,
    ) -> Result<RefreshToken, AuthError>;

    /// Check if a refresh token has been reused (security incident).
    async fn check_refresh_token_reuse(&self, token_hash: &[u8]) -> Result<bool, AuthError>;

    // ========== Organizations ==========

    /// Create a new organization.
    async fn create_org(
        &self,
        id: OrgId,
        slug: &str,
        name: &str,
        metadata: serde_json::Value,
    ) -> Result<Org, AuthError>;

    /// Find an organization by ID.
    async fn find_org_by_id(&self, id: &OrgId) -> Result<Option<Org>, AuthError>;

    /// Find an organization by slug.
    async fn find_org_by_slug(&self, slug: &str) -> Result<Option<Org>, AuthError>;

    /// Update an organization.
    async fn update_org(
        &self,
        id: &OrgId,
        slug: Option<&str>,
        name: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<Org, AuthError>;

    /// Delete an organization.
    async fn delete_org(&self, id: &OrgId) -> Result<(), AuthError>;

    /// List organizations for a user.
    async fn list_user_orgs(&self, user_id: &UserId) -> Result<Vec<Org>, AuthError>;

    // ========== Roles ==========

    /// Create a role.
    async fn create_role(
        &self,
        id: RoleId,
        org_id: &OrgId,
        name: &str,
        description: Option<&str>,
        is_system: bool,
        permissions: Vec<String>,
    ) -> Result<Role, AuthError>;

    /// Find a role by ID.
    async fn find_role_by_id(&self, id: &RoleId) -> Result<Option<Role>, AuthError>;

    /// Find a role by name in an org.
    async fn find_role_by_name(
        &self,
        org_id: &OrgId,
        name: &str,
    ) -> Result<Option<Role>, AuthError>;

    /// Update a role.
    async fn update_role(
        &self,
        id: &RoleId,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Role, AuthError>;

    /// Delete a role (fails if it's a system role).
    async fn delete_role(&self, id: &RoleId) -> Result<(), AuthError>;

    /// List roles for an organization.
    async fn list_org_roles(&self, org_id: &OrgId) -> Result<Vec<Role>, AuthError>;

    /// List roles for an organization with their permissions.
    async fn list_roles_with_permissions(
        &self,
        org_id: &OrgId,
    ) -> Result<Vec<(Role, Vec<String>)>, AuthError>;

    /// Get permissions for a role.
    async fn get_role_permissions(&self, role_id: &RoleId) -> Result<Vec<String>, AuthError>;

    /// Set permissions for a role (replaces existing).
    async fn set_role_permissions(
        &self,
        role_id: &RoleId,
        permissions: Vec<String>,
    ) -> Result<(), AuthError>;

    // ========== Memberships ==========

    /// Create a membership.
    async fn create_membership(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
        role_id: &RoleId,
    ) -> Result<Membership, AuthError>;

    /// Find a membership.
    async fn find_membership(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
    ) -> Result<Option<Membership>, AuthError>;

    /// Update a membership's role.
    async fn update_membership_role(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
        role_id: &RoleId,
    ) -> Result<Membership, AuthError>;

    /// Delete a membership.
    async fn delete_membership(&self, user_id: &UserId, org_id: &OrgId) -> Result<(), AuthError>;

    /// List members of an organization.
    async fn list_org_members(
        &self,
        org_id: &OrgId,
    ) -> Result<Vec<(User, Membership, Role)>, AuthError>;

    /// Count members with a specific role (for last-owner check).
    async fn count_members_with_role(
        &self,
        org_id: &OrgId,
        role_id: &RoleId,
    ) -> Result<u64, AuthError>;

    /// Get effective permissions for a user in an org.
    async fn get_user_permissions(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
    ) -> Result<Vec<String>, AuthError>;

    // ========== Invitations ==========

    /// Create an invitation.
    async fn create_invitation(
        &self,
        token_hash: &[u8],
        email: &str,
        org_id: &OrgId,
        role_id: &RoleId,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AuthError>;

    /// Find an invitation by token hash.
    async fn find_invitation_by_hash(
        &self,
        token_hash: &[u8],
    ) -> Result<Option<Invitation>, AuthError>;

    /// Mark an invitation as used.
    async fn use_invitation(&self, token_hash: &[u8]) -> Result<(), AuthError>;

    /// List pending invitations for an org.
    async fn list_org_invitations(&self, org_id: &OrgId) -> Result<Vec<Invitation>, AuthError>;

    /// Delete an invitation.
    async fn delete_invitation(&self, token_hash: &[u8]) -> Result<(), AuthError>;

    // ========== Signing Keys ==========

    /// Store a signing key.
    async fn store_signing_key(&self, key: &SigningKey) -> Result<(), AuthError>;

    /// Get the active signing key.
    async fn get_active_signing_key(&self) -> Result<Option<SigningKey>, AuthError>;

    /// Get all non-retired signing keys (for JWKS).
    async fn get_jwks_keys(&self) -> Result<Vec<SigningKey>, AuthError>;

    /// Rotate a signing key (set rotated_at on old, activate new).
    async fn rotate_signing_key(
        &self,
        old_kid: &str,
        new_key: &SigningKey,
    ) -> Result<(), AuthError>;

    /// Retire old signing keys (set retired_at on keys rotated > 7 days ago).
    async fn retire_old_signing_keys(&self) -> Result<u64, AuthError>;

    // ========== Verification Tokens ==========

    /// Create a verification token (for email verification, password reset, etc.).
    async fn create_verification_token(
        &self,
        user_id: &UserId,
        token_hash: &[u8],
        token_type: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<VerificationToken, AuthError>;

    /// Find a verification token by hash.
    async fn find_verification_token_by_hash(
        &self,
        token_hash: &[u8],
        token_type: &str,
    ) -> Result<Option<VerificationToken>, AuthError>;

    /// Mark a verification token as used.
    async fn use_verification_token(&self, token_hash: &[u8]) -> Result<(), AuthError>;

    /// Delete expired or used verification tokens for a user.
    async fn cleanup_verification_tokens(&self, user_id: &UserId) -> Result<u64, AuthError>;

    // ========== API Keys ==========

    /// Create an API key (stores only the hash, returns the full key once).
    async fn create_api_key(
        &self,
        id: reactor_core::ReactorId,
        user_id: &UserId,
        name: &str,
        key_hash: &[u8],
        prefix: &str,
        scopes: Option<Vec<String>>,
    ) -> Result<ApiKey, AuthError>;

    /// Find an API key by hash (for authentication).
    async fn find_api_key_by_hash(&self, key_hash: &[u8]) -> Result<Option<ApiKey>, AuthError>;

    /// List API keys for a user (excludes revoked).
    async fn list_user_api_keys(&self, user_id: &UserId) -> Result<Vec<ApiKey>, AuthError>;

    /// Revoke an API key.
    async fn revoke_api_key(
        &self,
        id: &reactor_core::ReactorId,
        user_id: &UserId,
    ) -> Result<(), AuthError>;

    /// Update last_used_at for an API key.
    async fn touch_api_key(&self, id: &reactor_core::ReactorId) -> Result<(), AuthError>;

    // ========== Audit Events ==========

    /// Log an audit event.
    async fn log_audit(&self, event: &AuditEvent) -> Result<(), AuthError>;

    // ========== Platform Operators ==========

    /// Grant a platform role to a user.
    async fn grant_platform_role(
        &self,
        user_id: &UserId,
        role_name: &str,
        granted_by: Option<&UserId>,
    ) -> Result<(), AuthError>;

    /// Revoke a platform role from a user.
    async fn revoke_platform_role(
        &self,
        user_id: &UserId,
        role_name: &str,
    ) -> Result<(), AuthError>;

    /// Check if a user has a platform role.
    async fn has_platform_role(
        &self,
        user_id: &UserId,
        role_name: &str,
    ) -> Result<bool, AuthError>;

    /// List all users with a specific platform role.
    async fn list_platform_role_users(
        &self,
        role_name: &str,
    ) -> Result<Vec<UserId>, AuthError>;

    /// Get platform permissions for a user.
    async fn get_platform_permissions(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<(String, bool)>, AuthError>;

    /// Check if any platform operators exist (for bootstrap check).
    async fn platform_operators_exist(&self) -> Result<bool, AuthError>;

    // ========== Authorization Codes (PKCE) ==========

    /// Create an authorization code.
    async fn create_authorization_code(
        &self,
        id: reactor_core::ReactorId,
        user_id: &UserId,
        client_id: &str,
        redirect_uri: &str,
        scopes: Vec<String>,
        code_hash: &[u8],
        code_challenge: &str,
        code_challenge_method: &str,
        nonce: Option<&str>,
        state: Option<&str>,
        expires_at: DateTime<Utc>,
        session_id: Option<&SessionId>,
    ) -> Result<AuthorizationCode, AuthError>;

    /// Find an authorization code by hash.
    async fn find_authorization_code_by_hash(
        &self,
        code_hash: &[u8],
    ) -> Result<Option<AuthorizationCode>, AuthError>;

    /// Mark an authorization code as used.
    async fn use_authorization_code(
        &self,
        code_hash: &[u8],
        session_id: &SessionId,
    ) -> Result<(), AuthError>;

    /// Clean up expired authorization codes.
    async fn cleanup_expired_authorization_codes(&self) -> Result<u64, AuthError>;
}
