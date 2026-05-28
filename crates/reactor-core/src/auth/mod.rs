//! Authentication and authorization types.
//!
//! This module defines the contract surface for Reactor's identity system:
//!
//! - [`Claims`] — JWT token claims
//! - [`AuthCtx`] — Full request authentication context
//! - [`AuthClient`] — Trait for consuming auth from other capabilities
//! - [`AuthError`] — Error types with stable error codes
//! - [`Jwks`] — JSON Web Key Set for token verification
//! - [`User`] — User entity type
//! - [`OrgRef`] — Organization reference (UUID or slug)

mod claims;
mod client;
mod error;
mod jwks;
pub mod permissions;
mod user;

use crate::id::OrgId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub use claims::{AuthCtx, AuthMethod, Claims, OauthProvider};
pub use client::AuthClient;
pub use error::AuthError;
pub use jwks::{JsonWebKey, Jwks};
pub use user::{User, UserSummary};

/// Organization reference — either a UUID or a slug.
///
/// When passed to `AuthClient::resolve_ctx`, the auth service resolves
/// the reference to an actual `OrgId`. UUID references are validated directly;
/// slug references require a database lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrgRef {
    /// Organization ID (UUID).
    Id(OrgId),
    /// Organization slug (human-readable identifier).
    Slug(String),
}

impl OrgRef {
    /// Create a new OrgRef from an OrgId.
    pub fn from_id(id: OrgId) -> Self {
        Self::Id(id)
    }

    /// Create a new OrgRef from a slug.
    pub fn from_slug(slug: impl Into<String>) -> Self {
        Self::Slug(slug.into())
    }

    /// Try to get the OrgId if this is an Id variant.
    pub fn as_id(&self) -> Option<&OrgId> {
        match self {
            Self::Id(id) => Some(id),
            Self::Slug(_) => None,
        }
    }

    /// Try to get the slug if this is a Slug variant.
    pub fn as_slug(&self) -> Option<&str> {
        match self {
            Self::Id(_) => None,
            Self::Slug(s) => Some(s),
        }
    }
}

impl FromStr for OrgRef {
    type Err = std::convert::Infallible;

    /// Parse a string as OrgRef.
    /// Tries UUID first, falls back to treating it as a slug.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<OrgId>() {
            Ok(id) => Ok(Self::Id(id)),
            Err(_) => Ok(Self::Slug(s.to_string())),
        }
    }
}

impl fmt::Display for OrgRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => write!(f, "{}", id),
            Self::Slug(s) => write!(f, "{}", s),
        }
    }
}

impl From<OrgId> for OrgRef {
    fn from(id: OrgId) -> Self {
        Self::Id(id)
    }
}

impl From<&OrgId> for OrgRef {
    fn from(id: &OrgId) -> Self {
        Self::Id(*id)
    }
}
