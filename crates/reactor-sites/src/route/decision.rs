//! Route decision resolver.

use super::matcher::{RouteMatcher, RouteMatch};
use crate::dispatch::RouteDecision;
use crate::store::DeploymentRoute;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// Route resolver that builds decisions from matches.
pub struct RouteResolver {
    matcher: RouteMatcher,
    function_map: HashMap<String, Uuid>,
}

impl RouteResolver {
    /// Create a new route resolver from deployment routes.
    pub fn from_routes(
        routes: &[DeploymentRoute],
        function_map: HashMap<String, Uuid>,
    ) -> Result<Self, matchit::InsertError> {
        let mut matcher = RouteMatcher::new();

        let mut sorted_routes = routes.to_vec();
        sorted_routes.sort_by(|a, b| b.priority.cmp(&a.priority));

        for route in &sorted_routes {
            let methods = route
                .method_filter
                .as_ref()
                .map(|m| m.split(',').map(|s| s.trim().to_string()).collect());

            matcher.add_route(
                &route.pattern,
                methods,
                super::matcher::RouteData {
                    kind: route.route_kind.clone(),
                    target: route.target_ref.clone(),
                    cache_rules: route.cache_rules_json.clone(),
                    priority: route.priority,
                },
            )?;
        }

        Ok(Self {
            matcher,
            function_map,
        })
    }

    /// Resolve a path and method to a route decision.
    pub fn resolve(&self, path: &str, method: &str) -> RouteDecision {
        let matched = match self.matcher.match_route(path, method) {
            Some(m) => m,
            None => return RouteDecision::NotFound,
        };

        self.build_decision(&matched)
    }

    fn build_decision(&self, matched: &RouteMatch<'_>) -> RouteDecision {
        let data = &matched.entry.data;

        match data.kind.as_str() {
            "static" => {
                let storage_key = self.expand_target(&data.target, &matched.params);
                let cache = serde_json::from_value(data.cache_rules.clone()).unwrap_or_default();

                RouteDecision::StaticFile {
                    storage_key,
                    cache,
                    content_type: None,
                }
            }
            "function" => {
                let function_name = data.target.clone();
                let function_id = self
                    .function_map
                    .get(&data.target)
                    .copied()
                    .unwrap_or_else(Uuid::nil);

                let sub_path = matched
                    .params
                    .get("path")
                    .or_else(|| matched.params.get("rest"))
                    .cloned()
                    .unwrap_or_default();

                RouteDecision::Function {
                    function_id,
                    function_name,
                    sub_path,
                }
            }
            "redirect" => {
                let location = self.expand_target(&data.target, &matched.params);

                let status = data.cache_rules.get("status")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(302) as u16;

                let permanent = status == 301 || status == 308;

                RouteDecision::Redirect {
                    location,
                    status,
                    permanent,
                }
            }
            "prerender" => {
                let storage_key = self.expand_target(&data.target, &matched.params);

                let revalidate = data.cache_rules.get("revalidate")
                    .and_then(|v| v.as_u64())
                    .map(Duration::from_secs);

                let fallback = data.cache_rules.get("fallback")
                    .and_then(|v| v.as_object())
                    .map(|f| {
                        let kind = f.get("kind").and_then(|k| k.as_str()).unwrap_or("function");
                        let target = f.get("target").and_then(|t| t.as_str()).unwrap_or("");

                        if kind == "function" {
                            let function_id = self
                                .function_map
                                .get(target)
                                .copied()
                                .unwrap_or_else(Uuid::nil);

                            Box::new(RouteDecision::Function {
                                function_id,
                                function_name: target.to_string(),
                                sub_path: matched
                                    .params
                                    .get("path")
                                    .cloned()
                                    .unwrap_or_default(),
                            })
                        } else {
                            Box::new(RouteDecision::NotFound)
                        }
                    });

                RouteDecision::Prerender {
                    storage_key,
                    revalidate_after: revalidate,
                    fallback,
                }
            }
            _ => RouteDecision::NotFound,
        }
    }

    fn expand_target(&self, target: &str, params: &HashMap<String, String>) -> String {
        let mut result = target.to_string();
        for (key, value) in params {
            result = result.replace(&format!("${}", key), value);
            result = result.replace(&format!(":{}", key), value);
        }
        result
    }
}
