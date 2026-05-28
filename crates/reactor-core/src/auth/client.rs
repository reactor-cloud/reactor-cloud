//! AuthClient trait — the contract for consuming auth from other capabilities.

use crate::auth::{AuthCtx, AuthError, Claims, Jwks, OrgRef, User};
use crate::id::UserId;
use async_trait::async_trait;

/// Client interface for authentication services.
///
/// This trait defines how other Reactor capabilities (Data, Storage, Functions, etc.)
/// interact with the auth system. It has two implementations:
///
/// - `InProcessAuthClient`: Direct calls to `AuthService`, zero overhead.
///   Used in G1 (Tauri) and G2 (single-server) deployments.
///
/// - `RemoteAuthClient`: HTTP calls to a separate auth service.
///   Used in G3 (microservices) deployments.
///
/// Consumer code depends only on this trait, never on concrete implementations.
/// The deployment topology is determined at startup.
#[async_trait]
pub trait AuthClient: Send + Sync + 'static {
    /// Verify a JWT token and return the claims.
    ///
    /// This performs cryptographic verification only — no database calls.
    /// Use `resolve_ctx` for full context including permissions.
    async fn verify_token(&self, token: &str) -> Result<Claims, AuthError>;

    /// Resolve the full authentication context for a request.
    ///
    /// This verifies the token, resolves the active organization
    /// (from `requested_org` or the token's `default_org`), validates
    /// membership, and loads effective permissions.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT access token from the Authorization header.
    /// * `requested_org` - The organization reference from `X-Reactor-Org` header.
    ///   Can be either a UUID or a slug; the auth service resolves slugs.
    async fn resolve_ctx(
        &self,
        token: &str,
        requested_org: Option<&OrgRef>,
    ) -> Result<AuthCtx, AuthError>;

    /// Get a user by ID.
    async fn get_user(&self, id: &UserId) -> Result<User, AuthError>;

    /// Check if the authenticated user has a specific permission.
    ///
    /// This evaluates the permission against the user's effective permissions
    /// in the active organization.
    async fn check_permission(&self, ctx: &AuthCtx, permission: &str) -> Result<bool, AuthError>;

    /// Get the current JWKS (JSON Web Key Set).
    ///
    /// This is primarily used by `RemoteAuthClient` to cache keys for
    /// local token verification.
    async fn jwks(&self) -> Result<Jwks, AuthError>;
}
