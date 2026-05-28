//! Path-to-regexp style route matching.

use matchit::Router;
use std::collections::HashMap;

/// Route matcher using matchit library.
pub struct RouteMatcher {
    router: Router<usize>,
    routes: Vec<RouteEntry>,
}

/// A single route entry.
#[derive(Debug, Clone)]
pub struct RouteEntry {
    /// Original pattern.
    pub pattern: String,
    /// Route index.
    pub index: usize,
    /// Method filter (None = all methods).
    pub methods: Option<Vec<String>>,
    /// Associated data.
    pub data: RouteData,
}

/// Data associated with a matched route.
#[derive(Debug, Clone)]
pub struct RouteData {
    /// Route kind: 'static', 'function', 'redirect', 'prerender'.
    pub kind: String,
    /// Target reference.
    pub target: String,
    /// Cache rules JSON.
    pub cache_rules: serde_json::Value,
    /// Priority.
    pub priority: i32,
}

/// Match result.
#[derive(Debug)]
pub struct RouteMatch<'a> {
    /// The matched route entry.
    pub entry: &'a RouteEntry,
    /// Path parameters extracted from the URL.
    pub params: HashMap<String, String>,
}

impl RouteMatcher {
    /// Create a new route matcher.
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            routes: Vec::new(),
        }
    }

    /// Add a route to the matcher.
    pub fn add_route(
        &mut self,
        pattern: &str,
        methods: Option<Vec<String>>,
        data: RouteData,
    ) -> Result<(), matchit::InsertError> {
        let index = self.routes.len();

        let matchit_pattern = convert_pattern(pattern);

        self.router.insert(&matchit_pattern, index)?;

        self.routes.push(RouteEntry {
            pattern: pattern.to_string(),
            index,
            methods,
            data,
        });

        Ok(())
    }

    /// Match a path and method.
    pub fn match_route<'a>(
        &'a self,
        path: &str,
        method: &str,
    ) -> Option<RouteMatch<'a>> {
        let matched = self.router.at(path).ok()?;

        let entry = &self.routes[*matched.value];

        if let Some(ref methods) = entry.methods {
            if !methods.iter().any(|m| m.eq_ignore_ascii_case(method)) {
                return None;
            }
        }

        let params: HashMap<String, String> = matched
            .params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        Some(RouteMatch { entry, params })
    }
}

impl Default for RouteMatcher {
    fn default() -> Self {
        Self::new()
    }
}

fn convert_pattern(pattern: &str) -> String {
    let mut result = String::new();

    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' {
            let mut param_name = String::new();
            let mut is_catch_all = false;
            while let Some(&next) = chars.peek() {
                if next.is_alphanumeric() || next == '_' {
                    param_name.push(chars.next().unwrap());
                } else if next == '*' || next == '+' {
                    // :param* or :param+ is a catch-all wildcard -> {*param}
                    chars.next();
                    is_catch_all = true;
                    break;
                } else {
                    break;
                }
            }
            if is_catch_all {
                result.push_str("{*");
                result.push_str(&param_name);
                result.push('}');
            } else {
                result.push('{');
                result.push_str(&param_name);
                result.push('}');
            }
        } else if c == '*' {
            result.push_str("{*rest}");
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_route_data(kind: &str, target: &str) -> RouteData {
        RouteData {
            kind: kind.to_string(),
            target: target.to_string(),
            cache_rules: json!({}),
            priority: 0,
        }
    }

    #[test]
    fn test_pattern_conversion_simple() {
        assert_eq!(convert_pattern("/"), "/");
        assert_eq!(convert_pattern("/foo"), "/foo");
        assert_eq!(convert_pattern("/foo/bar"), "/foo/bar");
    }

    #[test]
    fn test_pattern_conversion_params() {
        assert_eq!(convert_pattern("/:id"), "/{id}");
        assert_eq!(convert_pattern("/users/:id"), "/users/{id}");
        assert_eq!(convert_pattern("/users/:id/posts/:post_id"), "/users/{id}/posts/{post_id}");
    }

    #[test]
    fn test_pattern_conversion_wildcard() {
        assert_eq!(convert_pattern("/*"), "/{*rest}");
        assert_eq!(convert_pattern("/api/*"), "/api/{*rest}");
    }

    #[test]
    fn test_pattern_conversion_catch_all_param() {
        // :param* syntax for catch-all wildcards (common in Next.js/Vercel)
        assert_eq!(convert_pattern("/:path*"), "/{*path}");
        assert_eq!(convert_pattern("/_next/static/:path*"), "/_next/static/{*path}");
        // :param+ syntax (one-or-more) also converts to catch-all
        assert_eq!(convert_pattern("/:path+"), "/{*path}");
    }

    #[test]
    fn test_match_simple_path() {
        let mut matcher = RouteMatcher::new();
        matcher
            .add_route("/", None, test_route_data("static", "index.html"))
            .unwrap();
        matcher
            .add_route("/about", None, test_route_data("static", "about.html"))
            .unwrap();

        let result = matcher.match_route("/", "GET").unwrap();
        assert_eq!(result.entry.data.target, "index.html");

        let result = matcher.match_route("/about", "GET").unwrap();
        assert_eq!(result.entry.data.target, "about.html");

        assert!(matcher.match_route("/nonexistent", "GET").is_none());
    }

    #[test]
    fn test_match_with_params() {
        let mut matcher = RouteMatcher::new();
        matcher
            .add_route("/users/:id", None, test_route_data("function", "get_user"))
            .unwrap();

        let result = matcher.match_route("/users/123", "GET").unwrap();
        assert_eq!(result.entry.data.target, "get_user");
        assert_eq!(result.params.get("id"), Some(&"123".to_string()));

        let result = matcher.match_route("/users/abc", "GET").unwrap();
        assert_eq!(result.params.get("id"), Some(&"abc".to_string()));
    }

    #[test]
    fn test_match_with_wildcard() {
        let mut matcher = RouteMatcher::new();
        matcher
            .add_route("/api/*", None, test_route_data("function", "api_handler"))
            .unwrap();

        let result = matcher.match_route("/api/users", "GET").unwrap();
        assert_eq!(result.entry.data.target, "api_handler");

        let result = matcher.match_route("/api/users/123/posts", "GET").unwrap();
        assert_eq!(result.entry.data.target, "api_handler");
    }

    #[test]
    fn test_catch_all_does_not_match_root() {
        // IMPORTANT: matchit's {*path} catch-all does NOT match /
        // This is a known limitation. When generating routes, always add
        // an explicit "/" route alongside "/:path*" patterns.
        let mut matcher = RouteMatcher::new();
        matcher
            .add_route("/:path*", None, test_route_data("static", "catch_all"))
            .unwrap();

        // /:path* does NOT match / - this is a known matchit behavior
        assert!(matcher.match_route("/", "GET").is_none());

        // But it does match /foo
        let result = matcher.match_route("/foo", "GET").unwrap();
        assert_eq!(result.entry.data.target, "catch_all");
        assert_eq!(result.params.get("path"), Some(&"foo".to_string()));

        // And /foo/bar
        let result = matcher.match_route("/foo/bar", "GET").unwrap();
        assert_eq!(result.entry.data.target, "catch_all");
        assert_eq!(result.params.get("path"), Some(&"foo/bar".to_string()));
    }

    #[test]
    fn test_explicit_root_with_catch_all() {
        // To serve both / and /:path*, add an explicit / route
        let mut matcher = RouteMatcher::new();
        matcher
            .add_route("/", None, test_route_data("static", "index.html"))
            .unwrap();
        matcher
            .add_route("/:path*", None, test_route_data("static", "catch_all"))
            .unwrap();

        // / matches the explicit route
        let result = matcher.match_route("/", "GET").unwrap();
        assert_eq!(result.entry.data.target, "index.html");

        // /foo matches the catch-all
        let result = matcher.match_route("/foo", "GET").unwrap();
        assert_eq!(result.entry.data.target, "catch_all");
    }

    #[test]
    fn test_method_filter() {
        let mut matcher = RouteMatcher::new();
        matcher
            .add_route(
                "/api/resource",
                Some(vec!["GET".to_string(), "POST".to_string()]),
                test_route_data("function", "resource_handler"),
            )
            .unwrap();

        assert!(matcher.match_route("/api/resource", "GET").is_some());
        assert!(matcher.match_route("/api/resource", "POST").is_some());
        assert!(matcher.match_route("/api/resource", "get").is_some()); // case insensitive
        assert!(matcher.match_route("/api/resource", "DELETE").is_none());
    }

    #[test]
    fn test_no_method_filter_allows_all() {
        let mut matcher = RouteMatcher::new();
        matcher
            .add_route("/", None, test_route_data("static", "index.html"))
            .unwrap();

        assert!(matcher.match_route("/", "GET").is_some());
        assert!(matcher.match_route("/", "POST").is_some());
        assert!(matcher.match_route("/", "PUT").is_some());
        assert!(matcher.match_route("/", "DELETE").is_some());
        assert!(matcher.match_route("/", "OPTIONS").is_some());
    }

    #[test]
    fn test_multiple_params() {
        let mut matcher = RouteMatcher::new();
        matcher
            .add_route(
                "/orgs/:org_id/teams/:team_id/members/:member_id",
                None,
                test_route_data("function", "get_member"),
            )
            .unwrap();

        let result = matcher
            .match_route("/orgs/acme/teams/engineering/members/john", "GET")
            .unwrap();
        assert_eq!(result.params.get("org_id"), Some(&"acme".to_string()));
        assert_eq!(result.params.get("team_id"), Some(&"engineering".to_string()));
        assert_eq!(result.params.get("member_id"), Some(&"john".to_string()));
    }
}
