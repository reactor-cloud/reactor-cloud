//! Authentication and identity management for Reactor.cloud
//!
//! This crate provides:
//! - HTTP routes for the `/auth/v1/*` surface
//! - `AuthService` for business logic
//! - `IdentityStore` trait with Postgres implementation
//! - `InProcessAuthClient` and `RemoteAuthClient` implementations of `reactor_core::auth::AuthClient`
//!
//! See `docs/reactor-auth.design.md` for the full specification.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod client;
pub mod config;
pub mod crypto;
pub mod email;
pub mod error;
pub mod extract;
pub mod middleware;
pub mod password;
pub mod router;
pub mod routes;
pub mod service;
pub mod state;
pub mod store;
pub mod tenant;
pub mod token;
pub mod webauthn;

pub use client::{InProcessAuthClient, RemoteAuthClient};
pub use config::AuthConfig;
pub use crypto::{ColumnEncryptor, DataEncryptor, VaultEncryptor};
pub use email::{EmailSender, NoopSender, SmtpSender};
pub use extract::AuthBearer;
pub use middleware::{OrgContext, OrgContextLayer, RequestId, RequestIdLayer};
pub use password::PasswordHasherService;
pub use router::router;
pub use service::{AcceptInvitationResponse, AuthService};
pub use state::AuthState;
pub use store::{migrator, IdentityStore, PgIdentityStore};
pub use tenant::{OAuthConfigLoader, OAuthProviderConfig, TenantUrlBuilder};
pub use token::{KeyPair, Keyring, KeyringManager, TokenIssuer, TokenVerifier};
pub use webauthn::{WebAuthnProvider, WebAuthnStore, WebauthnError};

use utoipa::OpenApi;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// OpenAPI documentation for the auth service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor Auth API",
        version = "1.0.0",
        description = "Authentication and identity management API for Reactor.cloud"
    ),
    paths(
        routes::signup::signup,
        routes::login::login,
        routes::logout::logout,
        routes::token::token,
        routes::verify::verify_email,
        routes::verify::resend_verification,
        routes::user::get_user,
        routes::user::update_user,
        routes::user::delete_user,
        routes::orgs::create_org,
        routes::orgs::list_orgs,
        routes::orgs::get_org,
        routes::orgs::update_org,
        routes::orgs::delete_org,
        routes::orgs::list_roles,
        routes::members::list_members,
        routes::members::get_member,
        routes::members::update_member,
        routes::members::delete_member,
        routes::invitations::create_invitation,
        routes::invitations::list_invitations,
        routes::invitations::delete_invitation,
        routes::invitations::accept_invitation,
        routes::permissions::get_permissions,
        routes::permissions::check_permissions,
        routes::keys::jwks,
        routes::keys::openid_configuration,
        routes::password_reset::request_password_reset,
        routes::password_reset::confirm_password_reset,
        routes::api_keys::list_api_keys,
        routes::api_keys::create_api_key,
        routes::api_keys::revoke_api_key,
        routes::health::health,
    ),
    components(schemas(
        routes::signup::SignupRequest,
        routes::signup::SignupResponse,
        routes::signup::UserResponse,
        routes::signup::SessionResponse,
        routes::login::LoginRequest,
        routes::login::LoginResponse,
        routes::token::PasswordGrantRequest,
        routes::token::RefreshTokenRequest,
        routes::token::TokenResponse,
        routes::verify::ResendRequest,
        routes::verify::VerifyResponse,
        routes::verify::ResendResponse,
        routes::user::UpdateUserRequest,
        routes::orgs::CreateOrgRequest,
        routes::orgs::UpdateOrgRequest,
        routes::orgs::OrgResponse,
        routes::orgs::RoleResponse,
        routes::members::MemberResponse,
        routes::members::UpdateMemberRequest,
        routes::invitations::CreateInvitationRequest,
        routes::invitations::InvitationResponse,
        routes::permissions::CheckPermissionsRequest,
        routes::permissions::PermissionsResponse,
        routes::permissions::PermissionCheck,
        routes::keys::OidcDiscovery,
        routes::password_reset::PasswordResetRequestBody,
        routes::password_reset::PasswordResetRequestResponse,
        routes::password_reset::PasswordResetConfirmBody,
        routes::password_reset::PasswordResetConfirmResponse,
        routes::api_keys::CreateApiKeyRequest,
        routes::api_keys::CreateApiKeyResponse,
        routes::api_keys::ApiKeyResponse,
        routes::api_keys::ListApiKeysResponse,
        routes::api_keys::RevokeApiKeyResponse,
        routes::health::HealthResponse,
    )),
    tags(
        (name = "auth", description = "Core authentication endpoints"),
        (name = "auth.orgs", description = "Organization management"),
        (name = "auth.members", description = "Organization membership"),
        (name = "auth.invitations", description = "Organization invitations"),
        (name = "auth.permissions", description = "Permission checking"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

/// Returns the OpenAPI specification for the auth service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
