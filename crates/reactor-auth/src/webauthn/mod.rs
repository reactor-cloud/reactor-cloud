//! WebAuthn / Passkey support for MFA and passwordless authentication.
//!
//! This module provides:
//! - Credential registration (adding a passkey)
//! - Authentication (using a passkey for step-up or login)
//! - Credential management (list, rename, delete)
//!
//! Uses the `webauthn-rs` crate for FIDO2/WebAuthn protocol handling.

pub mod routes;
pub mod store;
pub mod types;

pub use routes::*;
pub use store::*;
pub use types::*;

use std::sync::Arc;
use webauthn_rs::prelude::*;

/// WebAuthn authenticator instance.
///
/// This wraps the `webauthn-rs` Webauthn struct with our configuration.
pub struct WebAuthnProvider {
    inner: Arc<Webauthn>,
}

impl WebAuthnProvider {
    /// Create a new WebAuthn provider.
    ///
    /// # Arguments
    /// - `rp_id` - Relying party ID (typically the domain, e.g., "reactor.cloud")
    /// - `rp_origin` - Relying party origin URL (e.g., "https://reactor.cloud")
    /// - `rp_name` - Human-readable name (e.g., "Reactor")
    pub fn new(rp_id: &str, rp_origin: &str, rp_name: &str) -> Result<Self, WebauthnError> {
        let rp_origin = Url::parse(rp_origin).map_err(|_| WebauthnError::Configuration)?;

        let builder = WebauthnBuilder::new(rp_id, &rp_origin)?
            .rp_name(rp_name)
            .allow_subdomains(true);

        let webauthn = builder.build()?;

        Ok(Self {
            inner: Arc::new(webauthn),
        })
    }

    /// Get a reference to the inner Webauthn instance.
    pub fn inner(&self) -> &Webauthn {
        &self.inner
    }

    /// Start a registration ceremony.
    ///
    /// Returns the creation challenge options to send to the client and
    /// the registration state to store temporarily.
    pub fn start_registration(
        &self,
        user_id: &[u8],
        user_name: &str,
        user_display_name: &str,
        existing_credentials: Vec<CredentialID>,
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), WebauthnError> {
        Ok(self.inner.start_passkey_registration(
            Uuid::from_slice(user_id).unwrap_or_else(|_| Uuid::new_v4()),
            user_name,
            user_display_name,
            Some(existing_credentials),
        )?)
    }

    /// Finish a registration ceremony.
    ///
    /// Validates the client response and returns the new passkey credential.
    pub fn finish_registration(
        &self,
        response: &RegisterPublicKeyCredential,
        state: &PasskeyRegistration,
    ) -> Result<Passkey, WebauthnError> {
        Ok(self.inner.finish_passkey_registration(response, state)?)
    }

    /// Start an authentication ceremony.
    ///
    /// Returns the request challenge options to send to the client and
    /// the authentication state to store temporarily.
    pub fn start_authentication(
        &self,
        credentials: &[Passkey],
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication), WebauthnError> {
        Ok(self.inner.start_passkey_authentication(credentials)?)
    }

    /// Finish an authentication ceremony.
    ///
    /// Validates the client response and returns the updated passkey
    /// (with incremented counter) and the credential ID that was used.
    pub fn finish_authentication(
        &self,
        response: &PublicKeyCredential,
        state: &PasskeyAuthentication,
    ) -> Result<AuthenticationResult, WebauthnError> {
        Ok(self.inner.finish_passkey_authentication(response, state)?)
    }
}

impl Clone for WebAuthnProvider {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// WebAuthn configuration or protocol error.
#[derive(Debug, thiserror::Error)]
pub enum WebauthnError {
    /// Configuration error (invalid RP ID, origin, etc.)
    #[error("WebAuthn configuration error")]
    Configuration,

    /// Protocol error from webauthn-rs.
    #[error("WebAuthn error: {0}")]
    Webauthn(#[from] webauthn_rs::prelude::WebauthnError),

    /// No credentials found for the user.
    #[error("No credentials found for user")]
    NoCredentials,

    /// Challenge not found or expired.
    #[error("Challenge not found or expired")]
    ChallengeNotFound,

    /// Credential not found.
    #[error("Credential not found")]
    CredentialNotFound,

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}
