//! Policy evaluation context trait.
//!
//! Defines the interface that request contexts must implement
//! to support policy evaluation.

use reactor_core::id::{OrgId, UserId};

/// Trait for contexts that can be used in policy evaluation.
///
/// This trait abstracts the auth context needed for evaluating policy expressions.
/// Both `DataCtx` (from reactor-data) and `StorageCtx` (from reactor-storage)
/// implement this trait.
pub trait PolicyEvalContext {
    /// Get the current user's ID, if authenticated.
    fn user_id(&self) -> Option<UserId>;

    /// Get the active organization ID.
    fn org_id(&self) -> Option<OrgId>;

    /// Check if the user has a specific permission.
    ///
    /// Permission strings follow the format `domain:resource:action`,
    /// with `*` as a wildcard that matches anything.
    fn has_permission(&self, permission: &str) -> bool;

    /// Get the user's email, if available.
    fn email(&self) -> Option<&str>;

    /// Get the session ID, if available.
    fn session_id(&self) -> Option<&str>;

    /// Check if the context represents an authenticated user.
    fn is_authenticated(&self) -> bool {
        self.user_id().is_some()
    }
}

/// A simple implementation for testing purposes.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct TestPolicyContext {
    pub user_id: Option<UserId>,
    pub org_id: Option<OrgId>,
    pub permissions: Vec<String>,
    pub email: Option<String>,
    pub session_id: Option<String>,
}

impl TestPolicyContext {
    /// Create a new test context with the given user and org.
    pub fn new(user_id: UserId, org_id: OrgId) -> Self {
        Self {
            user_id: Some(user_id),
            org_id: Some(org_id),
            permissions: vec![],
            email: None,
            session_id: None,
        }
    }

    /// Create an anonymous test context.
    pub fn anonymous() -> Self {
        Self::default()
    }

    /// Add a permission to the context.
    pub fn with_permission(mut self, permission: impl Into<String>) -> Self {
        self.permissions.push(permission.into());
        self
    }

    /// Add multiple permissions to the context.
    pub fn with_permissions(mut self, permissions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.permissions.extend(permissions.into_iter().map(Into::into));
        self
    }

    /// Set the email.
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }
}

impl PolicyEvalContext for TestPolicyContext {
    fn user_id(&self) -> Option<UserId> {
        self.user_id
    }

    fn org_id(&self) -> Option<OrgId> {
        self.org_id
    }

    fn has_permission(&self, permission: &str) -> bool {
        // Check for exact match or wildcard
        for p in &self.permissions {
            if p == "*" || p == permission {
                return true;
            }
            // Check for wildcard patterns like "data:*:read"
            if matches_permission_pattern(p, permission) {
                return true;
            }
        }
        false
    }

    fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }
}

/// Check if a permission pattern matches a specific permission.
fn matches_permission_pattern(pattern: &str, permission: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split(':').collect();
    let perm_parts: Vec<&str> = permission.split(':').collect();

    if pattern_parts.len() != perm_parts.len() {
        return false;
    }

    for (pat, perm) in pattern_parts.iter().zip(perm_parts.iter()) {
        if *pat != "*" && pat != perm {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_matching() {
        assert!(matches_permission_pattern("*", "anything"));
        assert!(matches_permission_pattern("data:*:read", "data:todos:read"));
        assert!(matches_permission_pattern("data:todos:*", "data:todos:write"));
        assert!(!matches_permission_pattern("data:todos:read", "data:todos:write"));
        assert!(!matches_permission_pattern("data:todos:read", "storage:todos:read"));
    }

    #[test]
    fn test_context_has_permission() {
        let ctx = TestPolicyContext::new(UserId::new(), OrgId::new())
            .with_permission("data:todos:read")
            .with_permission("data:*:write");

        assert!(ctx.has_permission("data:todos:read"));
        assert!(ctx.has_permission("data:anything:write"));
        assert!(!ctx.has_permission("data:todos:delete"));
    }

    #[test]
    fn test_wildcard_permission() {
        let ctx = TestPolicyContext::new(UserId::new(), OrgId::new())
            .with_permission("*");

        assert!(ctx.has_permission("anything:at:all"));
    }
}
