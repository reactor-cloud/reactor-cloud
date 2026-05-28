//! Unit tests for Caddy JSON config builder.

use reactor_core::{ProjectId, ProjectRef};
use reactor_gateway::{
    caddy_admin::{CaddyAdminConfig, CaddyConfigBuilder},
    routing::{BackendTarget, Route, RoutingTable},
};
use url::Url;

fn test_config() -> CaddyAdminConfig {
    CaddyAdminConfig::new(Url::parse("http://localhost:2019").unwrap())
        .with_cloudflare_token("test-cf-token")
        .with_default_backend("backend.internal:8000")
        .with_wildcard_domain("*.reactor.cloud")
        .with_acme_email("test@example.com")
}

fn test_route(host: &str) -> Route {
    let project_id = ProjectId::new();
    let project_ref = project_id.to_ref();
    Route::new(host, project_id, project_ref, BackendTarget::new("backend:8000"))
}

#[test]
fn test_empty_routing_table_produces_valid_config() {
    let config = test_config();
    let builder = CaddyConfigBuilder::new(&config);
    let table = RoutingTable::new();

    let result = builder.build(&table).unwrap();

    // Should have admin config
    assert!(result["admin"].is_object());
    
    // Should have apps section
    assert!(result["apps"]["http"].is_object());
    assert!(result["apps"]["tls"].is_object());
    
    // Should have servers
    assert!(result["apps"]["http"]["servers"]["https"].is_object());
    assert!(result["apps"]["http"]["servers"]["http"].is_object());
}

#[test]
fn test_single_route_added_to_config() {
    let config = test_config();
    let builder = CaddyConfigBuilder::new(&config);
    
    let mut table = RoutingTable::new();
    table.upsert(test_route("test.reactor.cloud"));

    let result = builder.build(&table).unwrap();
    let routes = result["apps"]["http"]["servers"]["https"]["routes"]
        .as_array()
        .unwrap();

    // Should have routes (including our route and default backend)
    assert!(!routes.is_empty());
    
    // Find our route
    let our_route = routes.iter().find(|r| {
        r["match"][0]["host"][0].as_str() == Some("test.reactor.cloud")
    });
    assert!(our_route.is_some());
}

#[test]
fn test_multiple_routes() {
    let config = test_config();
    let builder = CaddyConfigBuilder::new(&config);
    
    let mut table = RoutingTable::new();
    table.upsert(test_route("app1.reactor.cloud"));
    table.upsert(test_route("app2.reactor.cloud"));
    table.upsert(test_route("app3.reactor.cloud"));

    let result = builder.build(&table).unwrap();
    let routes = result["apps"]["http"]["servers"]["https"]["routes"]
        .as_array()
        .unwrap();

    // Should have at least 3 routes for our apps
    let app_routes: Vec<_> = routes
        .iter()
        .filter(|r| {
            let host = r["match"][0]["host"][0].as_str().unwrap_or_default();
            host.ends_with(".reactor.cloud") && !host.starts_with("*")
        })
        .collect();
    assert_eq!(app_routes.len(), 3);
}

#[test]
fn test_disabled_routes_excluded() {
    let config = test_config();
    let builder = CaddyConfigBuilder::new(&config);
    
    let mut table = RoutingTable::new();
    table.upsert(test_route("enabled.reactor.cloud"));
    table.upsert(test_route("disabled.reactor.cloud").disabled());

    let result = builder.build(&table).unwrap();
    let routes = result["apps"]["http"]["servers"]["https"]["routes"]
        .as_array()
        .unwrap();

    // Should have route for enabled but not disabled
    let enabled_route = routes.iter().find(|r| {
        r["match"][0]["host"][0].as_str() == Some("enabled.reactor.cloud")
    });
    let disabled_route = routes.iter().find(|r| {
        r["match"][0]["host"][0].as_str() == Some("disabled.reactor.cloud")
    });

    assert!(enabled_route.is_some());
    assert!(disabled_route.is_none());
}

#[test]
fn test_on_demand_tls_config() {
    let config = test_config();
    let builder = CaddyConfigBuilder::new(&config);
    let table = RoutingTable::new();

    let result = builder.build(&table).unwrap();
    let on_demand = &result["apps"]["tls"]["automation"]["on_demand"];

    // Should have on_demand TLS config
    assert_eq!(on_demand["ask"], "http://localhost:9000/ask");
    assert_eq!(on_demand["interval"], "2m");
    assert_eq!(on_demand["burst"], 5);
}

#[test]
fn test_acme_staging_config() {
    let config = test_config().with_acme_staging();
    let builder = CaddyConfigBuilder::new(&config);
    let table = RoutingTable::new();

    let result = builder.build(&table).unwrap();
    let policies = result["apps"]["tls"]["automation"]["policies"]
        .as_array()
        .unwrap();

    // At least one policy should have staging CA
    let has_staging = policies.iter().any(|p| {
        if let Some(issuers) = p["issuers"].as_array() {
            issuers.iter().any(|i| {
                i["ca"].as_str() == Some("https://acme-staging-v02.api.letsencrypt.org/directory")
            })
        } else {
            false
        }
    });
    assert!(has_staging);
}

#[test]
fn test_default_backend_route() {
    let config = test_config().with_default_backend("fallback.internal:9000");
    let builder = CaddyConfigBuilder::new(&config);
    let table = RoutingTable::new();

    let result = builder.build(&table).unwrap();
    let routes = result["apps"]["http"]["servers"]["https"]["routes"]
        .as_array()
        .unwrap();

    // Should have a catch-all route for default backend
    let default_route = routes.iter().find(|r| {
        r["match"][0]["host"][0].as_str() == Some("*")
    });
    assert!(default_route.is_some());
    
    let upstream = &default_route.unwrap()["handle"][0]["upstreams"][0]["dial"];
    assert_eq!(upstream.as_str().unwrap(), "fallback.internal:9000");
}

#[test]
fn test_reverse_proxy_headers() {
    let config = test_config();
    let builder = CaddyConfigBuilder::new(&config);
    
    let mut table = RoutingTable::new();
    table.upsert(test_route("test.reactor.cloud"));

    let result = builder.build(&table).unwrap();
    let routes = result["apps"]["http"]["servers"]["https"]["routes"]
        .as_array()
        .unwrap();

    // Find our route and check headers
    let our_route = routes.iter().find(|r| {
        r["match"][0]["host"][0].as_str() == Some("test.reactor.cloud")
    }).unwrap();

    let handler = &our_route["handle"][0];
    assert_eq!(handler["handler"], "reverse_proxy");
    
    // Should set forwarded headers
    let headers = &handler["headers"]["request"]["set"];
    assert!(headers["X-Forwarded-Proto"].is_array());
}

#[test]
fn test_http_redirect_server() {
    let config = test_config();
    let builder = CaddyConfigBuilder::new(&config);
    let table = RoutingTable::new();

    let result = builder.build(&table).unwrap();
    let http_server = &result["apps"]["http"]["servers"]["http"];

    // Should listen on port 80
    assert_eq!(http_server["listen"][0], ":80");

    // Should have redirect handler
    let route = &http_server["routes"][0];
    let handler = &route["handle"][0];
    assert_eq!(handler["handler"], "static_response");
    assert_eq!(handler["status_code"], "301");
}
