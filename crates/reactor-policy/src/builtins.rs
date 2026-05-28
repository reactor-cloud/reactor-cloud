//! Auth builtins for policy expressions.
//!
//! These are the `auth.*` functions that can be called in policy expressions.

use thiserror::Error;

/// Errors when validating builtin calls.
#[derive(Debug, Error)]
pub enum BuiltinError {
    #[error("unknown auth builtin: {0}")]
    UnknownBuiltin(String),

    #[error("auth.{name} expects {expected} arguments, got {got}")]
    ArityMismatch {
        name: String,
        expected: usize,
        got: usize,
    },

    #[error("auth.{name} argument {index} must be a string literal")]
    ArgumentMustBeString { name: String, index: usize },
}

/// Auth builtin function definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthBuiltin {
    /// `auth.user_id()` - Returns the current user's ID.
    UserId,

    /// `auth.org_id()` - Returns the current organization's ID.
    OrgId,

    /// `auth.role()` - Returns the user's role in the current org.
    Role,

    /// `auth.has_permission(permission)` - Checks if user has a permission.
    HasPermission,

    /// `auth.in_org(org_id)` - Checks if user is a member of the given org.
    InOrg,

    /// `auth.email()` - Returns the user's email.
    Email,

    /// `auth.session_id()` - Returns the current session ID.
    SessionId,

    /// `auth.is_authenticated()` - Returns true if user is authenticated.
    IsAuthenticated,
}

impl AuthBuiltin {
    /// Parse a builtin name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "user_id" => Some(Self::UserId),
            "org_id" => Some(Self::OrgId),
            "role" => Some(Self::Role),
            "has_permission" => Some(Self::HasPermission),
            "in_org" => Some(Self::InOrg),
            "email" => Some(Self::Email),
            "session_id" => Some(Self::SessionId),
            "is_authenticated" => Some(Self::IsAuthenticated),
            _ => None,
        }
    }

    /// Get the name of this builtin.
    pub fn name(&self) -> &'static str {
        match self {
            Self::UserId => "user_id",
            Self::OrgId => "org_id",
            Self::Role => "role",
            Self::HasPermission => "has_permission",
            Self::InOrg => "in_org",
            Self::Email => "email",
            Self::SessionId => "session_id",
            Self::IsAuthenticated => "is_authenticated",
        }
    }

    /// Get the expected number of arguments.
    pub fn arity(&self) -> usize {
        match self {
            Self::UserId
            | Self::OrgId
            | Self::Role
            | Self::Email
            | Self::SessionId
            | Self::IsAuthenticated => 0,
            Self::HasPermission | Self::InOrg => 1,
        }
    }

    /// Whether this builtin returns a boolean.
    pub fn returns_bool(&self) -> bool {
        match self {
            Self::UserId | Self::OrgId | Self::Role | Self::Email | Self::SessionId => false,
            Self::HasPermission | Self::InOrg | Self::IsAuthenticated => true,
        }
    }

    /// Whether this builtin's argument should be a string literal.
    pub fn argument_is_string(&self, index: usize) -> bool {
        match self {
            Self::HasPermission if index == 0 => true,
            Self::InOrg if index == 0 => false, // Can be column or literal
            _ => false,
        }
    }
}

/// Validate a builtin call.
pub fn validate_builtin_call(name: &str, arg_count: usize) -> Result<AuthBuiltin, BuiltinError> {
    let builtin = AuthBuiltin::from_name(name)
        .ok_or_else(|| BuiltinError::UnknownBuiltin(name.to_string()))?;

    if arg_count != builtin.arity() {
        return Err(BuiltinError::ArityMismatch {
            name: name.to_string(),
            expected: builtin.arity(),
            got: arg_count,
        });
    }

    Ok(builtin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_name() {
        assert_eq!(AuthBuiltin::from_name("user_id"), Some(AuthBuiltin::UserId));
        assert_eq!(AuthBuiltin::from_name("org_id"), Some(AuthBuiltin::OrgId));
        assert_eq!(AuthBuiltin::from_name("role"), Some(AuthBuiltin::Role));
        assert_eq!(
            AuthBuiltin::from_name("has_permission"),
            Some(AuthBuiltin::HasPermission)
        );
        assert_eq!(AuthBuiltin::from_name("in_org"), Some(AuthBuiltin::InOrg));
        assert_eq!(AuthBuiltin::from_name("email"), Some(AuthBuiltin::Email));
        assert_eq!(
            AuthBuiltin::from_name("is_authenticated"),
            Some(AuthBuiltin::IsAuthenticated)
        );
        assert_eq!(AuthBuiltin::from_name("unknown"), None);
    }

    #[test]
    fn test_validate_builtin_call() {
        assert!(validate_builtin_call("user_id", 0).is_ok());
        assert!(validate_builtin_call("user_id", 1).is_err());

        assert!(validate_builtin_call("has_permission", 1).is_ok());
        assert!(validate_builtin_call("has_permission", 0).is_err());
        assert!(validate_builtin_call("has_permission", 2).is_err());

        assert!(validate_builtin_call("unknown", 0).is_err());
    }
}
