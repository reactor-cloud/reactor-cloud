//! Permission string utilities.
//!
//! Permissions are colon-separated strings like `data:todos:read`.
//! Wildcards (`*`) can match any segment.

/// Check if a granted permission matches a requested permission.
///
/// Permissions are colon-separated. A `*` in the granted permission
/// matches any value in that segment of the requested permission.
///
/// # Examples
///
/// ```
/// use reactor_core::auth::permissions::matches;
///
/// assert!(matches("data:todos:read", "data:todos:read"));
/// assert!(matches("data:*:read", "data:todos:read"));
/// assert!(matches("data:todos:*", "data:todos:write"));
/// assert!(matches("*", "data:todos:read"));
/// assert!(!matches("data:todos:read", "data:todos:write"));
/// ```
#[must_use]
pub fn matches(granted: &str, requested: &str) -> bool {
    // Wildcard grants everything
    if granted == "*" {
        return true;
    }

    let granted_parts: Vec<&str> = granted.split(':').collect();
    let requested_parts: Vec<&str> = requested.split(':').collect();

    // Must have same number of segments (or granted has fewer with trailing *)
    if granted_parts.len() > requested_parts.len() {
        return false;
    }

    for (i, granted_part) in granted_parts.iter().enumerate() {
        if *granted_part == "*" {
            // Wildcard at end matches all remaining
            if i == granted_parts.len() - 1 {
                return true;
            }
            // Wildcard in middle matches this segment
            continue;
        }

        if i >= requested_parts.len() || *granted_part != requested_parts[i] {
            return false;
        }
    }

    // If granted has fewer parts than requested, it's not a match
    // unless the last part of granted is a wildcard (handled above)
    granted_parts.len() == requested_parts.len()
}

/// Check if any of the granted permissions match the requested permission.
#[must_use]
pub fn matches_any(granted: &[String], requested: &str) -> bool {
    granted.iter().any(|g| matches(g, requested))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(matches("data:todos:read", "data:todos:read"));
        assert!(!matches("data:todos:read", "data:todos:write"));
        assert!(!matches("data:todos:read", "data:users:read"));
    }

    #[test]
    fn test_wildcard_segment() {
        assert!(matches("data:*:read", "data:todos:read"));
        assert!(matches("data:*:read", "data:users:read"));
        assert!(!matches("data:*:read", "data:todos:write"));
    }

    #[test]
    fn test_trailing_wildcard() {
        assert!(matches("data:todos:*", "data:todos:read"));
        assert!(matches("data:todos:*", "data:todos:write"));
        assert!(matches("data:*:*", "data:todos:read"));
        assert!(!matches("data:todos:*", "data:users:read"));
    }

    #[test]
    fn test_global_wildcard() {
        assert!(matches("*", "data:todos:read"));
        assert!(matches("*", "storage:files:put"));
        assert!(matches("*", "anything"));
    }

    #[test]
    fn test_length_mismatch() {
        assert!(!matches("data:todos", "data:todos:read"));
        assert!(!matches("data:todos:read:extra", "data:todos:read"));
    }

    #[test]
    fn test_matches_any() {
        let perms = vec!["data:todos:read".to_string(), "data:users:*".to_string()];

        assert!(matches_any(&perms, "data:todos:read"));
        assert!(matches_any(&perms, "data:users:write"));
        assert!(!matches_any(&perms, "data:todos:write"));
        assert!(!matches_any(&perms, "storage:files:read"));
    }
}
