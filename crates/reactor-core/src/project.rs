//! Project identification types.
//!
//! - [`ProjectId`] — UUIDv7-based project identifier (immutable, internal)
//! - [`ProjectRef`] — URL-safe 20-character reference (derived from ProjectId, used in subdomains)

use crate::id::ReactorId;
use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

/// Error returned when parsing a ProjectRef fails.
#[derive(Debug, Clone, Error)]
pub enum ProjectRefError {
    /// Invalid length (must be 20 characters).
    #[error("invalid project ref length: expected 20 characters, got {0}")]
    InvalidLength(usize),
    /// Invalid characters (must be lowercase alphanumeric).
    #[error("invalid project ref: contains invalid characters")]
    InvalidCharacters,
    /// Failed to decode base32.
    #[error("invalid project ref encoding")]
    InvalidEncoding,
}

/// Project identifier — a UUIDv7-based ID for internal use.
///
/// This is the immutable primary key stored in databases and JWTs.
/// Use [`ProjectRef`] for user-facing URLs and subdomains.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(ReactorId);

impl ProjectId {
    /// Generate a new ProjectId.
    #[must_use]
    pub fn new() -> Self {
        Self(ReactorId::new())
    }

    /// Create a ProjectId from a ReactorId.
    #[must_use]
    pub const fn from_reactor_id(id: ReactorId) -> Self {
        Self(id)
    }

    /// Create a ProjectId from a raw UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(ReactorId::from_uuid(uuid))
    }

    /// Get the underlying ReactorId.
    #[must_use]
    pub const fn as_reactor_id(&self) -> &ReactorId {
        &self.0
    }

    /// Get the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> &Uuid {
        self.0.as_uuid()
    }

    /// Parse a ProjectId from a string.
    pub fn parse(s: &str) -> Result<Self, crate::id::ParseIdError> {
        ReactorId::parse(s).map(Self)
    }

    /// Create a nil (all-zeros) ProjectId.
    #[must_use]
    pub const fn nil() -> Self {
        Self(ReactorId::nil())
    }

    /// Check if this is a nil ID.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }

    /// Derive a [`ProjectRef`] from this ProjectId.
    ///
    /// The derivation is deterministic: the same ProjectId always produces
    /// the same ProjectRef. Uses blake3 hash of the UUID bytes, encoded as
    /// lowercase base32 (20 characters).
    #[must_use]
    pub fn to_ref(&self) -> ProjectRef {
        ProjectRef::from_project_id(self)
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProjectId({})", self.0)
    }
}

impl FromStr for ProjectId {
    type Err = crate::id::ParseIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<ReactorId> for ProjectId {
    fn from(id: ReactorId) -> Self {
        Self(id)
    }
}

impl From<ProjectId> for ReactorId {
    fn from(id: ProjectId) -> Self {
        id.0
    }
}

impl From<Uuid> for ProjectId {
    fn from(uuid: Uuid) -> Self {
        Self(ReactorId::from_uuid(uuid))
    }
}

impl From<ProjectId> for Uuid {
    fn from(id: ProjectId) -> Self {
        id.0.into_uuid()
    }
}

/// Project reference — a URL-safe 20-character identifier for subdomains.
///
/// Derived deterministically from a [`ProjectId`] using blake3 + base32.
/// Format: 20 lowercase alphanumeric characters (e.g., "abc123def456ghi789jk").
///
/// # Subdomain usage
///
/// ```text
/// {project_ref}.reactor.cloud  →  abc123def456ghi789jk.reactor.cloud
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectRef(String);

impl ProjectRef {
    /// Length of a ProjectRef in characters.
    pub const LENGTH: usize = 20;

    /// Derive a ProjectRef from a ProjectId.
    ///
    /// Uses blake3 hash of the UUID bytes, then takes the first 12.5 bytes
    /// and encodes as base32 (20 characters). The result is lowercased.
    #[must_use]
    pub fn from_project_id(id: &ProjectId) -> Self {
        let uuid_bytes = id.as_uuid().as_bytes();
        let hash = blake3::hash(uuid_bytes);
        let hash_bytes = hash.as_bytes();

        // Take first 12.5 bytes (100 bits) → 20 base32 characters
        // We take 13 bytes and truncate the encoded output
        let encoded = BASE32_NOPAD.encode(&hash_bytes[..13]);
        let truncated = &encoded[..Self::LENGTH];

        Self(truncated.to_lowercase())
    }

    /// Parse a ProjectRef from a string.
    ///
    /// Validates that the string is exactly 20 lowercase alphanumeric characters.
    pub fn parse(s: &str) -> Result<Self, ProjectRefError> {
        if s.len() != Self::LENGTH {
            return Err(ProjectRefError::InvalidLength(s.len()));
        }

        // Must be lowercase alphanumeric
        if !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
            return Err(ProjectRefError::InvalidCharacters);
        }

        Ok(Self(s.to_string()))
    }

    /// Get the ref as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }

    /// Create a ProjectRef from a string without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the string is a valid 20-character lowercase
    /// alphanumeric string. This is useful when loading from a trusted source
    /// like the database.
    #[must_use]
    pub fn from_string_unchecked(s: String) -> Self {
        debug_assert!(s.len() == Self::LENGTH);
        debug_assert!(s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        Self(s)
    }
}

impl fmt::Display for ProjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for ProjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProjectRef({})", self.0)
    }
}

impl FromStr for ProjectRef {
    type Err = ProjectRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl AsRef<str> for ProjectRef {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Type alias for [`ProjectRef`] when used as a URL slug.
pub type ProjectSlug = ProjectRef;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_id_new() {
        let id = ProjectId::new();
        assert!(!id.is_nil());
    }

    #[test]
    fn test_project_id_roundtrip() {
        let id = ProjectId::new();
        let s = id.to_string();
        let parsed = ProjectId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_project_ref_deterministic() {
        let id = ProjectId::new();
        let ref1 = id.to_ref();
        let ref2 = id.to_ref();
        assert_eq!(ref1, ref2);
    }

    #[test]
    fn test_project_ref_length() {
        let id = ProjectId::new();
        let ref_ = id.to_ref();
        assert_eq!(ref_.as_str().len(), ProjectRef::LENGTH);
    }

    #[test]
    fn test_project_ref_lowercase_alphanumeric() {
        let id = ProjectId::new();
        let ref_ = id.to_ref();
        assert!(ref_
            .as_str()
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_project_ref_parse_valid() {
        let ref_ = ProjectRef::parse("abc123def456ghi789jk").unwrap();
        assert_eq!(ref_.as_str(), "abc123def456ghi789jk");
    }

    #[test]
    fn test_project_ref_parse_invalid_length() {
        let result = ProjectRef::parse("abc");
        assert!(matches!(result, Err(ProjectRefError::InvalidLength(3))));
    }

    #[test]
    fn test_project_ref_parse_invalid_chars() {
        let result = ProjectRef::parse("ABC123DEF456GHI789JK");
        assert!(matches!(result, Err(ProjectRefError::InvalidCharacters)));
    }

    #[test]
    fn test_project_ref_unique_for_different_ids() {
        let id1 = ProjectId::new();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let id2 = ProjectId::new();

        let ref1 = id1.to_ref();
        let ref2 = id2.to_ref();

        assert_ne!(ref1, ref2);
    }

    #[test]
    fn test_project_id_serde() {
        let id = ProjectId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: ProjectId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_project_ref_serde() {
        let ref_ = ProjectRef::parse("abc123def456ghi789jk").unwrap();
        let json = serde_json::to_string(&ref_).unwrap();
        let parsed: ProjectRef = serde_json::from_str(&json).unwrap();
        assert_eq!(ref_, parsed);
    }
}
