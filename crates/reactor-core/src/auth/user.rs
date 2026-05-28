//! User types for the auth module.

use crate::id::{OrgId, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A user in the Reactor system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique user identifier.
    pub id: UserId,

    /// User's email address.
    pub email: String,

    /// Whether the email has been verified.
    pub email_verified: bool,

    /// User's default organization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_org_id: Option<OrgId>,

    /// Custom metadata stored with the user.
    #[serde(default)]
    pub metadata: serde_json::Value,

    /// When the user was created.
    pub created_at: DateTime<Utc>,

    /// When the user was last updated.
    pub updated_at: DateTime<Utc>,

    /// When the user was disabled (if disabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<DateTime<Utc>>,
}

impl User {
    /// Check if the user account is disabled.
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled_at.is_some()
    }
}

/// Summary of a user for inclusion in responses.
///
/// Excludes sensitive fields that shouldn't be exposed in all contexts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    /// Unique user identifier.
    pub id: UserId,

    /// User's email address.
    pub email: String,

    /// Custom metadata stored with the user.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl From<User> for UserSummary {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            metadata: user.metadata,
        }
    }
}
