//! Cross-capability integration test harness for reactor-sites.
//!
//! Tests the full site lifecycle:
//! signup → create site → upload bundle → deploy → promote → serve → assert audit + invocations
//!
//! Matrix: {static, hono, nextjs} × {InProcess, Remote}

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use chrono::Utc;
use reactor_core::{
    auth::{AuthClient, AuthCtx, AuthError, Claims, Jwks, OrgRef, User},
    id::{OrgId, UserId},
};
use reactor_sites::{router, SitesConfig, SitesState, PgSitesStore};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

/// Test configuration.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TestConfig {
    framework: FrameworkType,
    auth_mode: AuthMode,
}

/// Framework type for the test.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum FrameworkType {
    Static,
    Hono,
    Nextjs,
}

/// Auth mode for the test.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum AuthMode {
    InProcess,
    Remote,
}

/// Generate test matrix configurations.
#[allow(dead_code)]
fn generate_test_matrix() -> Vec<TestConfig> {
    let mut configs = Vec::new();

    for framework in [FrameworkType::Static, FrameworkType::Hono, FrameworkType::Nextjs] {
        for auth_mode in [AuthMode::InProcess, AuthMode::Remote] {
            configs.push(TestConfig {
                framework,
                auth_mode,
            });
        }
    }

    configs
}

/// A mock AuthClient that allows all requests.
struct MockAuthClient {
    user_id: UserId,
    org_id: OrgId,
}

impl MockAuthClient {
    fn new() -> Self {
        Self {
            user_id: UserId::new(),
            org_id: OrgId::new(),
        }
    }
}

#[async_trait]
impl AuthClient for MockAuthClient {
    async fn verify_token(&self, _token: &str) -> Result<Claims, AuthError> {
        Ok(Claims {
            sub: format!("user_{}", self.user_id),
            iat: Utc::now().timestamp(),
            exp: i64::MAX,
            iss: "reactor-auth".to_string(),
            aud: "reactor".to_string(),
            nbf: None,
            email: Some("test@example.com".to_string()),
            amr: vec![],
            orgs: vec![],
            default_org: None,
            session_id: None,
            scopes: vec![],
            mfa_at: None,
        })
    }

    async fn resolve_ctx(
        &self,
        _token: &str,
        _requested_org: Option<&OrgRef>,
    ) -> Result<AuthCtx, AuthError> {
        Ok(AuthCtx {
            claims: Claims {
                sub: format!("user_{}", self.user_id),
                iat: Utc::now().timestamp(),
                exp: i64::MAX,
                iss: "reactor-auth".to_string(),
                aud: "reactor".to_string(),
                nbf: None,
                email: Some("test@example.com".to_string()),
                amr: vec![],
                orgs: vec![],
                default_org: None,
                session_id: None,
                scopes: vec![],
                mfa_at: None,
            },
            active_org: Some(self.org_id),
            permissions: vec!["*".to_string()],
        })
    }

    async fn get_user(&self, id: &UserId) -> Result<User, AuthError> {
        Ok(User {
            id: *id,
            email: "test@example.com".to_string(),
            email_verified: true,
            default_org_id: None,
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            disabled_at: None,
        })
    }

    async fn check_permission(&self, _ctx: &AuthCtx, _permission: &str) -> Result<bool, AuthError> {
        Ok(true)
    }

    async fn jwks(&self) -> Result<Jwks, AuthError> {
        Ok(Jwks { keys: vec![] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_generation() {
        let configs = generate_test_matrix();
        // 3 frameworks × 2 auth modes = 6
        assert_eq!(configs.len(), 6);
    }

    // TODO: Implement full integration tests
    // These tests require:
    // 1. testcontainers for Postgres
    // 2. Mock functions service
    // 3. Mock storage service
    // 4. Auth service or in-process auth
    //
    // Test flow:
    // 1. Start containers (Postgres)
    // 2. Run migrations
    // 3. Create auth user + org
    // 4. Create site
    // 5. Upload bundle
    // 6. Wait for deployment to become ready
    // 7. Promote deployment
    // 8. Serve request via Host header
    // 9. Assert audit_events table has records
    // 10. Assert invocations table has records

    #[tokio::test]
    #[ignore = "requires testcontainers setup"]
    async fn test_static_site_inprocess() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup"]
    async fn test_static_site_remote() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup and bun"]
    async fn test_hono_site_inprocess() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup and bun"]
    async fn test_hono_site_remote() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup and node"]
    async fn test_nextjs_site_inprocess() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup and node"]
    async fn test_nextjs_site_remote() {
        // TODO: Implement
    }
}
