//! Reactor ID types — UUIDv7-based identifiers for all entities.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

/// Error returned when parsing a ReactorId fails.
#[derive(Debug, Clone, Error)]
#[error("invalid reactor id: {0}")]
pub struct ParseIdError(String);

/// A UUIDv7-based identifier used for all Reactor entities.
///
/// UUIDv7 is time-sortable, which provides:
/// - Natural ordering by creation time
/// - Better database index locality
/// - Rough timestamp extraction without additional fields
///
/// Display format is the canonical 36-character lowercase hyphenated UUID.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReactorId(Uuid);

impl ReactorId {
    /// Generate a new UUIDv7-based ReactorId.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Create a ReactorId from a raw UUID.
    ///
    /// This does not validate that the UUID is v7 — use when loading from storage.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get the underlying UUID, consuming self.
    #[must_use]
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }

    /// Parse a ReactorId from a string.
    ///
    /// Accepts the canonical 36-character hyphenated format.
    pub fn parse(s: &str) -> Result<Self, ParseIdError> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| ParseIdError(e.to_string()))
    }

    /// Create a nil (all-zeros) ReactorId.
    ///
    /// Useful for testing or as a sentinel value.
    #[must_use]
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Check if this is a nil ID.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for ReactorId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ReactorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_hyphenated())
    }
}

impl fmt::Debug for ReactorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReactorId({})", self.0.as_hyphenated())
    }
}

impl FromStr for ReactorId {
    type Err = ParseIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<Uuid> for ReactorId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ReactorId> for Uuid {
    fn from(id: ReactorId) -> Self {
        id.0
    }
}

impl AsRef<Uuid> for ReactorId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

/// User identifier.
pub type UserId = ReactorId;

/// Organization identifier.
pub type OrgId = ReactorId;

/// Session identifier.
pub type SessionId = ReactorId;

/// Role identifier.
pub type RoleId = ReactorId;

/// Invitation identifier.
pub type InvitationId = ReactorId;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_generates_v7() {
        let id = ReactorId::new();
        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn test_display_roundtrip() {
        let id = ReactorId::new();
        let s = id.to_string();
        let parsed = ReactorId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_serde_roundtrip() {
        let id = ReactorId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: ReactorId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_nil() {
        let id = ReactorId::nil();
        assert!(id.is_nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn test_parse_error() {
        let result = ReactorId::parse("not-a-uuid");
        assert!(result.is_err());
    }

    #[test]
    fn test_ordering_by_time() {
        let id1 = ReactorId::new();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let id2 = ReactorId::new();
        assert!(id1.to_string() < id2.to_string());
    }
}
