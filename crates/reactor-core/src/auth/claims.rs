//! JWT claims and authentication context types.

use crate::id::{OrgId, SessionId, UserId};
use serde::{Deserialize, Serialize};

/// OAuth provider for external identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OauthProvider {
    /// Google OAuth.
    Google,
    /// GitHub OAuth.
    Github,
}

/// Authentication method used to establish a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// Password authentication.
    Pwd,
    /// TOTP two-factor authentication.
    Totp,
    /// OAuth provider authentication.
    Oauth(OauthProvider),
    /// API key authentication.
    Apikey,
    /// Magic link authentication.
    MagicLink,
}

/// JWT claims for Reactor access tokens.
///
/// Claims follow the JWT standard with Reactor-specific extensions
/// for multi-tenancy support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — `"user_<id>"` for users, `"apikey:<id>"` for API keys.
    pub sub: String,

    /// Issuer — always `"reactor-auth"`.
    pub iss: String,

    /// Audience — always `"reactor"`.
    pub aud: String,

    /// Expiration time (Unix timestamp in seconds).
    pub exp: i64,

    /// Issued at time (Unix timestamp in seconds).
    pub iat: i64,

    /// Not before time (Unix timestamp in seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,

    /// User's email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Authentication methods used for this session.
    #[serde(default)]
    pub amr: Vec<AuthMethod>,

    /// Organizations the user belongs to.
    #[serde(default)]
    pub orgs: Vec<OrgId>,

    /// User's default organization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_org: Option<OrgId>,

    /// Session ID (absent for API keys).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,

    /// Granted scopes for this token (e.g., "ops:deploy", "cloud:*").
    /// Empty means no special scopes (normal user access).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,

    /// Unix timestamp (seconds) when MFA was last verified.
    /// Used for step-up authentication requirements.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mfa_at: Option<i64>,
}

impl Claims {
    /// Extract the user ID from the subject claim.
    ///
    /// Returns `None` if the subject is not a user (e.g., it's an API key).
    #[must_use]
    pub fn user_id(&self) -> Option<UserId> {
        self.sub
            .strip_prefix("user_")
            .and_then(|id| id.parse().ok())
    }

    /// Check if this token is for an API key rather than a user session.
    #[must_use]
    pub fn is_apikey(&self) -> bool {
        self.sub.starts_with("apikey:")
    }

    /// Check if the token has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.exp < now
    }

    /// Check if the user is a member of the given organization.
    #[must_use]
    pub fn is_member_of(&self, org_id: &OrgId) -> bool {
        self.orgs.contains(org_id)
    }

    /// Check if the token has a specific scope.
    #[must_use]
    pub fn has_scope(&self, scope: &str) -> bool {
        crate::auth::permissions::matches_any(&self.scopes, scope)
    }

    /// Check if MFA was verified recently (within the given window in seconds).
    ///
    /// Returns `true` if `mfa_at` is set and is within the window.
    #[must_use]
    pub fn mfa_verified_within(&self, window_secs: i64) -> bool {
        if let Some(mfa_at) = self.mfa_at {
            let now = chrono::Utc::now().timestamp();
            now - mfa_at <= window_secs
        } else {
            false
        }
    }

    /// Check if step-up authentication is required for a given scope.
    ///
    /// Returns `true` if MFA was NOT verified within the window.
    #[must_use]
    pub fn requires_step_up(&self, window_secs: i64) -> bool {
        !self.mfa_verified_within(window_secs)
    }
}

/// Full authentication context for a request.
///
/// This combines the verified JWT claims with the resolved organization
/// context and permissions for the current request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCtx {
    /// Verified JWT claims.
    pub claims: Claims,

    /// Active organization for this request.
    ///
    /// Resolved from `X-Reactor-Org` header or `claims.default_org`.
    pub active_org: Option<OrgId>,

    /// Effective permissions for the user in the active organization.
    ///
    /// Pre-resolved from the user's role memberships.
    pub permissions: Vec<String>,
}

impl AuthCtx {
    /// Get the user ID from the claims.
    ///
    /// Returns `None` if this is an API key token.
    #[must_use]
    pub fn user_id(&self) -> Option<UserId> {
        self.claims.user_id()
    }

    /// Get the active organization ID.
    ///
    /// Returns `None` if no organization context is set.
    #[must_use]
    pub fn org_id(&self) -> Option<OrgId> {
        self.active_org
    }

    /// Check if the user has a specific permission in the active org.
    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        crate::auth::permissions::matches_any(&self.permissions, permission)
    }
}
