//! Policy scope enumeration.
//!
//! Defines the operations that policies can apply to.

use serde::{Deserialize, Serialize};

/// Policy scope — the operation type a policy applies to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PolicyScope {
    /// SELECT operations (reading data).
    Select,
    /// INSERT operations (creating data).
    Insert,
    /// UPDATE operations (modifying data).
    Update,
    /// DELETE operations (removing data).
    Delete,
    /// Read operations (for storage: GET/HEAD).
    Read,
    /// Write operations (for storage: PUT/POST).
    Write,
}

impl PolicyScope {
    /// Get the string representation of this scope.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Read => "read",
            Self::Write => "write",
        }
    }

    /// Parse a scope from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "select" => Some(Self::Select),
            "insert" => Some(Self::Insert),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            _ => None,
        }
    }

    /// Check if this is a read operation.
    pub fn is_read(&self) -> bool {
        matches!(self, Self::Select | Self::Read)
    }

    /// Check if this is a write operation.
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Insert | Self::Update | Self::Write)
    }

    /// Check if this is a delete operation.
    pub fn is_delete(&self) -> bool {
        matches!(self, Self::Delete)
    }
}

impl std::fmt::Display for PolicyScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_as_str() {
        assert_eq!(PolicyScope::Select.as_str(), "select");
        assert_eq!(PolicyScope::Insert.as_str(), "insert");
        assert_eq!(PolicyScope::Update.as_str(), "update");
        assert_eq!(PolicyScope::Delete.as_str(), "delete");
        assert_eq!(PolicyScope::Read.as_str(), "read");
        assert_eq!(PolicyScope::Write.as_str(), "write");
    }

    #[test]
    fn test_scope_from_str() {
        assert_eq!(PolicyScope::from_str("select"), Some(PolicyScope::Select));
        assert_eq!(PolicyScope::from_str("SELECT"), Some(PolicyScope::Select));
        assert_eq!(PolicyScope::from_str("read"), Some(PolicyScope::Read));
        assert_eq!(PolicyScope::from_str("unknown"), None);
    }

    #[test]
    fn test_scope_categories() {
        assert!(PolicyScope::Select.is_read());
        assert!(PolicyScope::Read.is_read());
        assert!(!PolicyScope::Insert.is_read());

        assert!(PolicyScope::Insert.is_write());
        assert!(PolicyScope::Update.is_write());
        assert!(PolicyScope::Write.is_write());
        assert!(!PolicyScope::Delete.is_write());

        assert!(PolicyScope::Delete.is_delete());
        assert!(!PolicyScope::Select.is_delete());
    }
}
