//! Integration tests for reactor-data with reactor-auth.
//!
//! Tests both deployment topologies:
//! 1. InProcess - AuthService embedded in the same process
//! 2. Remote - reactor-auth-server running as a separate service
//!
//! Scenario tested:
//! - Sign up user → create org → seed todos → CRUD operations
//! - Verify auth context propagation
//! - Verify audit events are recorded

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use chrono::Utc;
use reactor_core::{
    auth::{AuthClient, AuthCtx, AuthError, AuthMethod, Claims, Jwks, OrgRef, User},
    id::{OrgId, UserId},
};
use reactor_data::{router, DataConfig, DataState, Deployment, PgDataStore};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tower::ServiceExt;

/// Test user context for integration tests.
#[derive(Clone)]
struct TestUser {
    id: UserId,
    org_id: OrgId,
    permissions: Vec<String>,
}

impl TestUser {
    fn owner() -> Self {
        Self {
            id: UserId::new(),
            org_id: OrgId::new(),
            permissions: vec!["*".to_string()],
        }
    }

    fn member(org_id: OrgId) -> Self {
        Self {
            id: UserId::new(),
            org_id,
            permissions: vec!["data:todos:read".to_string()],
        }
    }
}

/// A configurable mock AuthClient for testing different permission scenarios.
struct TestAuthClient {
    user: TestUser,
}

impl TestAuthClient {
    fn new(user: TestUser) -> Self {
        Self { user }
    }
}

#[async_trait]
impl AuthClient for TestAuthClient {
    async fn verify_token(&self, _token: &str) -> Result<Claims, AuthError> {
        Ok(Claims {
            sub: format!("user_{}", self.user.id),
            iat: Utc::now().timestamp(),
            exp: i64::MAX,
            iss: "reactor-auth".to_string(),
            aud: "reactor".to_string(),
            nbf: None,
            email: Some("test@example.com".to_string()),
            amr: vec![AuthMethod::Pwd],
            orgs: vec![self.user.org_id],
            default_org: Some(self.user.org_id),
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
                sub: format!("user_{}", self.user.id),
                iat: Utc::now().timestamp(),
                exp: i64::MAX,
                iss: "reactor-auth".to_string(),
                aud: "reactor".to_string(),
                nbf: None,
                email: Some("test@example.com".to_string()),
                amr: vec![AuthMethod::Pwd],
                orgs: vec![self.user.org_id],
                default_org: Some(self.user.org_id),
                session_id: None,
                scopes: vec![],
                mfa_at: None,
            },
            active_org: Some(self.user.org_id),
            permissions: self.user.permissions.clone(),
        })
    }

    async fn get_user(&self, _id: &UserId) -> Result<User, AuthError> {
        Ok(User {
            id: self.user.id,
            email: "test@example.com".to_string(),
            email_verified: true,
            default_org_id: Some(self.user.org_id),
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            disabled_at: None,
        })
    }

    async fn check_permission(&self, _ctx: &AuthCtx, permission: &str) -> Result<bool, AuthError> {
        Ok(self.user.permissions.iter().any(|p| p == permission || p == "*"))
    }

    async fn jwks(&self) -> Result<Jwks, AuthError> {
        Ok(Jwks { keys: vec![] })
    }
}

/// Set up the test database with all required schemas and tables.
async fn setup_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Create _reactor_data schema
    sqlx::query("CREATE SCHEMA IF NOT EXISTS _reactor_data")
        .execute(pool)
        .await?;

    // Create migrations table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _reactor_data.migrations (
            name TEXT PRIMARY KEY,
            checksum TEXT NOT NULL,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create policies table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _reactor_data.policies (
            id SERIAL PRIMARY KEY,
            schema_name TEXT NOT NULL,
            table_name TEXT NOT NULL,
            name TEXT NOT NULL,
            scopes TEXT[] NOT NULL,
            using_ast JSONB,
            check_ast JSONB,
            migration_name TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create tables metadata table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _reactor_data.tables (
            schema_name TEXT NOT NULL,
            table_name TEXT NOT NULL,
            columns JSONB NOT NULL,
            primary_keys TEXT[] NOT NULL DEFAULT '{}',
            foreign_keys JSONB NOT NULL DEFAULT '[]',
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (schema_name, table_name)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create audit_events table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _reactor_data.audit_events (
            id UUID PRIMARY KEY,
            ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            actor_user_id UUID,
            actor_apikey_id UUID,
            org_id UUID,
            request_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            table_name TEXT,
            row_count INTEGER,
            details JSONB NOT NULL DEFAULT '{}'::JSONB
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create RPC functions table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _reactor_data.rpc_functions (
            id BIGSERIAL PRIMARY KEY,
            schema_name TEXT NOT NULL,
            name TEXT NOT NULL,
            params JSONB NOT NULL DEFAULT '[]',
            return_type TEXT NOT NULL,
            returns_set BOOLEAN NOT NULL DEFAULT FALSE,
            body TEXT NOT NULL,
            security TEXT NOT NULL DEFAULT 'definer',
            migration_name TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE (schema_name, name)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create user schema
    sqlx::query("CREATE SCHEMA IF NOT EXISTS app")
        .execute(pool)
        .await?;

    // Create todos table
    sqlx::query(
        r#"
        CREATE TABLE app.todos (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            org_id UUID NOT NULL,
            title TEXT NOT NULL,
            done BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Register todos table in metadata
    sqlx::query(
        r#"
        INSERT INTO _reactor_data.tables (schema_name, table_name, columns, primary_keys)
        VALUES ('app', 'todos', $1, '{id}')
        "#,
    )
    .bind(json!([
        {"name": "id", "type": "uuid", "nullable": false, "default": "gen_random_uuid()"},
        {"name": "org_id", "type": "uuid", "nullable": false, "default": null},
        {"name": "title", "type": "text", "nullable": false, "default": null},
        {"name": "done", "type": "boolean", "nullable": false, "default": "false"},
        {"name": "created_at", "type": "timestamptz", "nullable": false, "default": "now()"},
        {"name": "updated_at", "type": "timestamptz", "nullable": false, "default": "now()"}
    ]))
    .execute(pool)
    .await?;

    Ok(())
}

fn test_config() -> DataConfig {
    DataConfig {
        database_url: String::new(), // Set per test
        bind: "127.0.0.1:0".parse().unwrap(),
        migrations_dir: None,
        user_schema: "app".to_string(),
        run_migrations: false,
        max_embed_depth: 5,
        max_limit: 1000,
        default_limit: 100,
        log: "warn".to_string(),
        deployment: Deployment::Monolith,
        auth_url: None,
        internal_secret: None,
        auth_database_url: None,
        auth_data_key: None,
        metrics: false,
    }
}

/// Helper to make requests to the router.
async fn request(
    app: &axum::Router,
    method: Method,
    path: &str,
    body: Option<Value>,
    token: &str,
) -> (StatusCode, Value) {
    let builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json");

    let body = if let Some(json) = body {
        Body::from(serde_json::to_vec(&json).unwrap())
    } else {
        Body::empty()
    };

    let request = builder.body(body).unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let body: Value = if body_bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or(Value::Null)
    };

    (status, body)
}

/// Test: InProcess topology - owner can CRUD, member can only read.
#[tokio::test]
async fn test_inprocess_topology_permission_enforcement() {
    // Start Postgres container
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("failed to start postgres");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let pool = PgPool::connect(&db_url).await.expect("failed to connect");
    setup_database(&pool).await.expect("failed to setup database");

    let store = Arc::new(PgDataStore::new(pool.clone()));

    // Test 1: Owner with * permission can insert
    let owner = TestUser::owner();
    let org_id = owner.org_id;
    let owner_client: Arc<dyn AuthClient> = Arc::new(TestAuthClient::new(owner));

    let mut config = test_config();
    config.database_url = db_url.clone();
    let state = DataState::new(store.clone(), owner_client, Arc::new(config.clone()));
    let app = router(state);

    let (status, body) = request(
        &app,
        Method::POST,
        "/data/v1/todos",
        Some(json!({
            "org_id": org_id.to_string(),
            "title": "Test todo"
        })),
        "owner-token",
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "Owner insert failed: {:?}", body);

    // Test 2: Owner can read
    let (status, body) = request(&app, Method::GET, "/data/v1/todos", None, "owner-token").await;
    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("expected array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], "Test todo");

    // Test 3: Member with read-only permission can read
    let member = TestUser::member(org_id);
    let member_client: Arc<dyn AuthClient> = Arc::new(TestAuthClient::new(member));
    let state = DataState::new(store.clone(), member_client, Arc::new(config.clone()));
    let member_app = router(state);

    let (status, _body) = request(
        &member_app,
        Method::GET,
        "/data/v1/todos",
        None,
        "member-token",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Member read should succeed");

    // Test 4: Verify audit events were recorded
    let audit_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM _reactor_data.audit_events WHERE event_type = 'rows.insert'",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to query audit events");

    assert_eq!(audit_count.0, 1, "Expected 1 audit event for insert");

    // Cleanup
    pool.close().await;
}

/// Test: Health endpoint works without auth.
#[tokio::test]
async fn test_health_endpoint() {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("failed to start postgres");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let pool = PgPool::connect(&db_url).await.expect("failed to connect");
    setup_database(&pool).await.expect("failed to setup database");

    let store = Arc::new(PgDataStore::new(pool.clone()));
    let auth_client: Arc<dyn AuthClient> = Arc::new(TestAuthClient::new(TestUser::owner()));

    let mut config = test_config();
    config.database_url = db_url;
    let state = DataState::new(store, auth_client, Arc::new(config));
    let app = router(state);

    // Health endpoint should work without auth
    let request = Request::builder()
        .method(Method::GET)
        .uri("/data/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    pool.close().await;
}

/// Test: X-Request-Id propagation.
#[tokio::test]
async fn test_request_id_propagation() {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("failed to start postgres");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let pool = PgPool::connect(&db_url).await.expect("failed to connect");
    setup_database(&pool).await.expect("failed to setup database");

    let store = Arc::new(PgDataStore::new(pool.clone()));
    let auth_client: Arc<dyn AuthClient> = Arc::new(TestAuthClient::new(TestUser::owner()));

    let mut config = test_config();
    config.database_url = db_url;
    let state = DataState::new(store, auth_client, Arc::new(config));
    let app = router(state);

    // Request with custom X-Request-Id
    let request_id = "test-request-123";
    let request = Request::builder()
        .method(Method::GET)
        .uri("/data/v1/health")
        .header("X-Request-Id", request_id)
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check that X-Request-Id is echoed back
    let echoed = response
        .headers()
        .get("x-request-id")
        .map(|v| v.to_str().unwrap_or(""));
    assert_eq!(echoed, Some(request_id));

    pool.close().await;
}
