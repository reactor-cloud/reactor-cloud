//! Bundle manifest schema and validation.

use crate::error::SitesError;
use crate::Framework;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Site bundle manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Site name (must match the site being deployed to).
    pub name: String,

    /// Version (server-assigned on deploy).
    #[serde(default)]
    pub version: i64,

    /// Framework that produced this bundle.
    pub framework: Framework,

    /// Route definitions (ordered, first match wins).
    pub routes: Vec<BundleRoute>,

    /// Function configurations.
    #[serde(default)]
    pub functions: HashMap<String, FunctionConfig>,

    /// Redirect definitions.
    #[serde(default)]
    pub redirects: Vec<ManifestRedirect>,

    /// Header rules.
    #[serde(default)]
    pub headers: Vec<HeaderRule>,

    /// Environment variable keys (non-secret).
    #[serde(default)]
    pub env_keys: Vec<String>,

    /// Secret environment variable keys.
    #[serde(default)]
    pub secret_keys: Vec<String>,

    /// Analytics configuration.
    #[serde(default)]
    pub analytics: Option<AnalyticsConfig>,
}

/// Analytics configuration for a site.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsConfig {
    /// Enable analytics for this site.
    #[serde(default)]
    pub enabled: bool,

    /// Auto-inject snippet into HTML responses.
    #[serde(default)]
    pub inject_snippet: bool,

    /// Auto-capture pageviews.
    #[serde(default = "default_auto_pageview")]
    pub auto_pageview: bool,

    /// Auto-capture errors.
    #[serde(default)]
    pub auto_errors: bool,

    /// Auto-capture clicks (opt-in).
    #[serde(default)]
    pub auto_capture: bool,

    /// Custom selector for auto-capture.
    #[serde(default)]
    pub auto_capture_selector: Option<String>,
}

fn default_auto_pageview() -> bool {
    true
}

/// Route definition in manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRoute {
    /// Path pattern (e.g., "/api/:path*").
    pub pattern: String,

    /// Route kind.
    pub kind: RouteKind,

    /// Target reference (storage path, function name, redirect URL).
    pub target: String,

    /// HTTP methods (optional, defaults to all).
    #[serde(default)]
    pub methods: Option<Vec<String>>,

    /// Cache rules (for static routes).
    #[serde(default)]
    pub cache: Option<CacheRules>,

    /// Fallback route (for prerender routes).
    #[serde(default)]
    pub fallback: Option<Box<BundleRoute>>,

    /// Revalidate interval in seconds (for prerender routes).
    #[serde(default)]
    pub revalidate: Option<u64>,

    /// Tags for invalidation (for prerender routes).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Route kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteKind {
    /// Static file serving.
    Static,
    /// Function invocation.
    Function,
    /// HTTP redirect.
    Redirect,
    /// Prerendered content with ISR.
    Prerender,
}

impl std::fmt::Display for RouteKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteKind::Static => write!(f, "static"),
            RouteKind::Function => write!(f, "function"),
            RouteKind::Redirect => write!(f, "redirect"),
            RouteKind::Prerender => write!(f, "prerender"),
        }
    }
}

/// Cache rules for static content.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheRules {
    /// max-age in seconds.
    #[serde(default)]
    pub max_age: Option<u64>,

    /// s-maxage in seconds (CDN cache).
    #[serde(default)]
    pub s_maxage: Option<u64>,

    /// stale-while-revalidate in seconds.
    #[serde(default)]
    pub stale_while_revalidate: Option<u64>,

    /// Whether the content is immutable.
    #[serde(default)]
    pub immutable: bool,
}

impl CacheRules {
    /// Convert to Cache-Control header value.
    pub fn to_cache_control(&self) -> String {
        let mut parts = Vec::new();

        if let Some(max_age) = self.max_age {
            parts.push(format!("max-age={}", max_age));
        }

        if let Some(s_maxage) = self.s_maxage {
            parts.push(format!("s-maxage={}", s_maxage));
        }

        if let Some(swr) = self.stale_while_revalidate {
            parts.push(format!("stale-while-revalidate={}", swr));
        }

        if self.immutable {
            parts.push("immutable".to_string());
        }

        if parts.is_empty() {
            "public, max-age=0, must-revalidate".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Function configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionConfig {
    /// Runtime: 'wasm', 'bun', or 'lambda'.
    pub runtime: String,

    /// Entry point file.
    pub entrypoint: String,

    /// Resource limits.
    #[serde(default)]
    pub limits: FunctionLimits,
}

/// Function resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionLimits {
    /// Timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Memory limit in MB.
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
}

fn default_timeout_ms() -> u64 {
    30_000
}

fn default_memory_mb() -> u32 {
    256
}

impl Default for FunctionLimits {
    fn default() -> Self {
        Self {
            timeout_ms: default_timeout_ms(),
            memory_mb: default_memory_mb(),
        }
    }
}

/// Redirect definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestRedirect {
    /// Source path pattern.
    pub source: String,

    /// Destination URL or path.
    pub destination: String,

    /// Whether this is a permanent redirect (301 vs 302).
    #[serde(default)]
    pub permanent: bool,
}

/// Header rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderRule {
    /// Path pattern.
    pub pattern: String,

    /// Headers to add.
    pub headers: HashMap<String, String>,
}

impl Manifest {
    /// Validate the manifest.
    pub fn validate(&self) -> Result<(), SitesError> {
        if self.name.is_empty() {
            return Err(SitesError::ManifestInvalid("name is required".to_string()));
        }

        if !crate::SITE_NAME_REGEX.is_match(&self.name) {
            return Err(SitesError::ManifestInvalid(format!(
                "invalid site name: {}",
                self.name
            )));
        }

        if self.routes.is_empty() {
            return Err(SitesError::ManifestInvalid(
                "at least one route is required".to_string(),
            ));
        }

        for route in &self.routes {
            self.validate_route(route)?;
        }

        for (name, config) in &self.functions {
            if config.entrypoint.is_empty() {
                return Err(SitesError::ManifestInvalid(format!(
                    "function '{}' missing entrypoint",
                    name
                )));
            }

            if !["wasm", "bun", "lambda"].contains(&config.runtime.as_str()) {
                return Err(SitesError::ManifestInvalid(format!(
                    "function '{}' has invalid runtime: {}",
                    name, config.runtime
                )));
            }
        }

        Ok(())
    }

    fn validate_route(&self, route: &BundleRoute) -> Result<(), SitesError> {
        if route.pattern.is_empty() {
            return Err(SitesError::ManifestInvalid(
                "route pattern cannot be empty".to_string(),
            ));
        }

        if route.target.is_empty() {
            return Err(SitesError::ManifestInvalid(format!(
                "route '{}' missing target",
                route.pattern
            )));
        }

        match route.kind {
            RouteKind::Function => {
                if !self.functions.contains_key(&route.target) {
                    return Err(SitesError::ManifestInvalid(format!(
                        "route '{}' references unknown function: {}",
                        route.pattern, route.target
                    )));
                }
            }
            RouteKind::Prerender => {
                if let Some(ref fallback) = route.fallback {
                    self.validate_route(fallback)?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Framework;

    fn valid_static_manifest() -> Manifest {
        Manifest {
            name: "my-site".to_string(),
            version: 0,
            framework: Framework::Static,
            routes: vec![BundleRoute {
                pattern: "/*".to_string(),
                kind: RouteKind::Static,
                target: "$rest".to_string(),
                methods: None,
                cache: None,
                fallback: None,
                revalidate: None,
                tags: vec![],
            }],
            functions: HashMap::new(),
            redirects: vec![],
            headers: vec![],
            env_keys: vec![],
            secret_keys: vec![],
            analytics: None,
        }
    }

    #[test]
    fn test_valid_static_manifest() {
        let manifest = valid_static_manifest();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_empty_name_fails() {
        let mut manifest = valid_static_manifest();
        manifest.name = "".to_string();
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name is required"));
    }

    #[test]
    fn test_invalid_name_fails() {
        let mut manifest = valid_static_manifest();
        manifest.name = "My Site!".to_string(); // Invalid: spaces and special chars
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid site name"));
    }

    #[test]
    fn test_empty_routes_fails() {
        let mut manifest = valid_static_manifest();
        manifest.routes = vec![];
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one route"));
    }

    #[test]
    fn test_empty_route_pattern_fails() {
        let mut manifest = valid_static_manifest();
        manifest.routes[0].pattern = "".to_string();
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("pattern cannot be empty"));
    }

    #[test]
    fn test_empty_route_target_fails() {
        let mut manifest = valid_static_manifest();
        manifest.routes[0].target = "".to_string();
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing target"));
    }

    #[test]
    fn test_function_route_unknown_function_fails() {
        let mut manifest = valid_static_manifest();
        manifest.routes = vec![BundleRoute {
            pattern: "/api/*".to_string(),
            kind: RouteKind::Function,
            target: "nonexistent".to_string(),
            methods: None,
            cache: None,
            fallback: None,
            revalidate: None,
            tags: vec![],
        }];
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown function"));
    }

    #[test]
    fn test_function_route_valid() {
        let mut manifest = valid_static_manifest();
        manifest.functions.insert(
            "api".to_string(),
            FunctionConfig {
                runtime: "bun".to_string(),
                entrypoint: "index.js".to_string(),
                limits: FunctionLimits::default(),
            },
        );
        manifest.routes = vec![BundleRoute {
            pattern: "/api/*".to_string(),
            kind: RouteKind::Function,
            target: "api".to_string(),
            methods: None,
            cache: None,
            fallback: None,
            revalidate: None,
            tags: vec![],
        }];
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_invalid_runtime_fails() {
        let mut manifest = valid_static_manifest();
        manifest.functions.insert(
            "api".to_string(),
            FunctionConfig {
                runtime: "invalid".to_string(),
                entrypoint: "index.js".to_string(),
                limits: FunctionLimits::default(),
            },
        );
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid runtime"));
    }

    #[test]
    fn test_empty_entrypoint_fails() {
        let mut manifest = valid_static_manifest();
        manifest.functions.insert(
            "api".to_string(),
            FunctionConfig {
                runtime: "bun".to_string(),
                entrypoint: "".to_string(),
                limits: FunctionLimits::default(),
            },
        );
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing entrypoint"));
    }

    #[test]
    fn test_redirect_route_valid() {
        let mut manifest = valid_static_manifest();
        manifest.routes.push(BundleRoute {
            pattern: "/old".to_string(),
            kind: RouteKind::Redirect,
            target: "/new".to_string(),
            methods: None,
            cache: None,
            fallback: None,
            revalidate: None,
            tags: vec![],
        });
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_prerender_with_fallback_valid() {
        let mut manifest = valid_static_manifest();
        manifest.routes = vec![BundleRoute {
            pattern: "/posts/:slug".to_string(),
            kind: RouteKind::Prerender,
            target: "posts/".to_string(),
            methods: None,
            cache: None,
            fallback: Some(Box::new(BundleRoute {
                pattern: "/posts/:slug".to_string(),
                kind: RouteKind::Static,
                target: "404.html".to_string(),
                methods: None,
                cache: None,
                fallback: None,
                revalidate: None,
                tags: vec![],
            })),
            revalidate: Some(60),
            tags: vec!["posts".to_string()],
        }];
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_manifest_json_roundtrip() {
        let manifest = valid_static_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, manifest.name);
        assert_eq!(parsed.routes.len(), manifest.routes.len());
    }

    #[test]
    fn test_function_limits_defaults() {
        let limits = FunctionLimits::default();
        assert_eq!(limits.timeout_ms, 30_000);
        assert_eq!(limits.memory_mb, 256);
    }
}
