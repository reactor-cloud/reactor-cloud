//! Integration tests for CRUD operations.
//!
//! Tests the full CRUD cycle with various Prefer header permutations.

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
use reactor_data::{router, DataConfig, DataState, PgDataStore};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tower::ServiceExt;

/// A mock AuthClient that allows all requests.
struct MockAuthClient;

#[async_trait]
impl AuthClient for MockAuthClient {
    async fn verify_token(&self, _token: &str) -> Result<Claims, AuthError> {
        let user_id = UserId::new();
        Ok(Claims {
            sub: format!("user_{}", user_id),
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
        let user_id = UserId::new();
        Ok(AuthCtx {
            claims: Claims {
                sub: format!("user_{}", user_id),
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
            active_org: Some(OrgId::new()),
            permissions: vec!["*".to_string()], // Allow all permissions
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

/// Set up the test database with metadata tables and a todos table.
async fn setup_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Create metadata schema
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

    // Create user schema
    sqlx::query("CREATE SCHEMA IF NOT EXISTS app")
        .execute(pool)
        .await?;

    // Create todos table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS app.todos (
            id SERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            completed BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Register in _reactor_data.tables
    sqlx::query(
        r#"
        INSERT INTO _reactor_data.tables (schema_name, table_name, columns, primary_keys, foreign_keys)
        VALUES (
            'app',
            'todos',
            '[
                {"name": "id", "data_type": "integer", "is_nullable": false},
                {"name": "title", "data_type": "text", "is_nullable": false},
                {"name": "completed", "data_type": "boolean", "is_nullable": false},
                {"name": "created_at", "data_type": "timestamp with time zone", "is_nullable": false}
            ]'::jsonb,
            ARRAY['id'],
            '[]'::jsonb
        )
        ON CONFLICT (schema_name, table_name) DO UPDATE
        SET columns = EXCLUDED.columns,
            primary_keys = EXCLUDED.primary_keys,
            foreign_keys = EXCLUDED.foreign_keys,
            updated_at = NOW()
        "#,
    )
    .execute(pool)
    .await?;

    // Seed some data
    sqlx::query(
        r#"
        INSERT INTO app.todos (title, completed) VALUES
        ('Buy groceries', false),
        ('Clean house', false),
        ('Write tests', true)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Create test state with the given pool.
fn create_test_state(pool: PgPool, schema: &str) -> DataState<PgDataStore> {
    let store = Arc::new(PgDataStore::new(pool));
    let config = Arc::new(DataConfig {
        database_url: "postgres://test".to_string(),
        bind: "127.0.0.1:8080".parse().unwrap(),
        migrations_dir: None,
        run_migrations: false,
        user_schema: schema.to_string(),
        max_embed_depth: 5,
        max_limit: 1000,
        default_limit: 100,
        deployment: reactor_data::Deployment::Monolith,
        auth_url: None,
        internal_secret: None,
        auth_database_url: Some("postgres://test".to_string()), // Required for monolith
        auth_data_key: Some("test_key_0000000000000000000000".to_string()), // Required for monolith
        log: "debug".to_string(),
        metrics: false,
    });
    let auth: Arc<dyn AuthClient> = Arc::new(MockAuthClient);

    DataState {
        config,
        store,
        auth,
    }
}

/// Helper to make a request to the test router.
async fn make_request(
    router: axum::Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    headers: Vec<(&str, &str)>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut req_builder = Request::builder().method(method).uri(uri);

    for (key, value) in headers {
        req_builder = req_builder.header(key, value);
    }

    // Always add a bearer token for auth middleware
    req_builder = req_builder.header("Authorization", "Bearer test-token");

    let body = match body {
        Some(v) => {
            req_builder = req_builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_string(&v).unwrap())
        }
        None => Body::empty(),
    };

    let req = req_builder.body(body).unwrap();
    let response = router.oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body: Value = if body_bytes.is_empty() {
        json!(null)
    } else {
        serde_json::from_slice(&body_bytes)
            .unwrap_or_else(|_| json!({"raw": String::from_utf8_lossy(&body_bytes).to_string()}))
    };

    (status, headers, body)
}

#[tokio::test]
async fn test_crud_select_all() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, body) =
        make_request(app, Method::GET, "/data/v1/todos", None, vec![]).await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 3);
}

#[tokio::test]
async fn test_crud_select_with_filter() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, body) = make_request(
        app,
        Method::GET,
        "/data/v1/todos?completed=eq.true",
        None,
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], "Write tests");
}

#[tokio::test]
async fn test_crud_select_with_order() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, body) = make_request(
        app,
        Method::GET,
        "/data/v1/todos?order=title.desc",
        None,
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["title"], "Write tests");
    assert_eq!(rows[1]["title"], "Clean house");
    assert_eq!(rows[2]["title"], "Buy groceries");
}

#[tokio::test]
async fn test_crud_select_with_pagination() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, body) = make_request(
        app,
        Method::GET,
        "/data/v1/todos?limit=2&offset=1&order=id",
        None,
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["title"], "Clean house");
    assert_eq!(rows[1]["title"], "Write tests");
}

#[tokio::test]
async fn test_crud_select_with_count() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, headers, body) = make_request(
        app,
        Method::GET,
        "/data/v1/todos?limit=1",
        None,
        vec![("Prefer", "count=exact")],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 1);

    // Check Content-Range header
    let content_range = headers
        .get("Content-Range")
        .expect("Expected Content-Range header")
        .to_str()
        .unwrap();
    assert!(
        content_range.contains("3"),
        "Content-Range should show total count of 3: {}",
        content_range
    );
}

#[tokio::test]
async fn test_crud_insert() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, _body) = make_request(
        app,
        Method::POST,
        "/data/v1/todos",
        Some(json!({"title": "New todo", "completed": false})),
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn test_crud_insert_with_representation() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, body) = make_request(
        app,
        Method::POST,
        "/data/v1/todos",
        Some(json!({"title": "New todo", "completed": false})),
        vec![("Prefer", "return=representation")],
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], "New todo");
    assert!(rows[0]["id"].is_number());
}

#[tokio::test]
async fn test_crud_update() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, _body) = make_request(
        app,
        Method::PATCH,
        "/data/v1/todos?id=eq.1",
        Some(json!({"completed": true})),
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_crud_update_with_representation() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, body) = make_request(
        app,
        Method::PATCH,
        "/data/v1/todos?id=eq.1",
        Some(json!({"completed": true})),
        vec![("Prefer", "return=representation")],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["completed"], true);
}

#[tokio::test]
async fn test_crud_delete() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool.clone(), "app");
    let app = router(state);

    let (status, _headers, _body) =
        make_request(app, Method::DELETE, "/data/v1/todos?id=eq.1", None, vec![]).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify deletion
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM app.todos WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn test_crud_delete_with_representation() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, body) = make_request(
        app,
        Method::DELETE,
        "/data/v1/todos?id=eq.1",
        None,
        vec![("Prefer", "return=representation")],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], 1);
}

#[tokio::test]
async fn test_crud_select_columns() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, body) = make_request(
        app,
        Method::GET,
        "/data/v1/todos?select=id,title",
        None,
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    assert_eq!(rows.len(), 3);

    // Check that only selected columns are present
    for row in rows {
        assert!(row.get("id").is_some());
        assert!(row.get("title").is_some());
        assert!(row.get("completed").is_none());
        assert!(row.get("created_at").is_none());
    }
}

#[tokio::test]
async fn test_invalid_table_returns_404() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, _body) =
        make_request(app, Method::GET, "/data/v1/nonexistent", None, vec![]).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_invalid_column_returns_400() {
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    let state = create_test_state(pool, "app");
    let app = router(state);

    let (status, _headers, _body) = make_request(
        app,
        Method::GET,
        "/data/v1/todos?select=nonexistent_column",
        None,
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ============================================================================
// Policy Enforcement Tests
// ============================================================================

/// A restricted AuthClient that provides specific permissions (not `*`).
struct RestrictedMockAuthClient {
    permissions: Vec<String>,
    user_id: UserId,
    org_id: OrgId,
}

impl RestrictedMockAuthClient {
    fn new(permissions: Vec<String>) -> Self {
        Self {
            permissions,
            user_id: UserId::new(),
            org_id: OrgId::new(),
        }
    }
}

#[async_trait]
impl AuthClient for RestrictedMockAuthClient {
    async fn verify_token(&self, _token: &str) -> Result<Claims, AuthError> {
        Ok(Claims {
            sub: format!("user_{}", self.user_id),
            iat: Utc::now().timestamp(),
            exp: i64::MAX,
            iss: "reactor-auth".to_string(),
            aud: "reactor".to_string(),
            nbf: None,
            email: Some("restricted@example.com".to_string()),
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
                email: Some("restricted@example.com".to_string()),
                amr: vec![],
                orgs: vec![],
                default_org: None,
                session_id: None,
                scopes: vec![],
                mfa_at: None,
            },
            active_org: Some(self.org_id),
            permissions: self.permissions.clone(),
        })
    }

    async fn get_user(&self, id: &UserId) -> Result<User, AuthError> {
        Ok(User {
            id: *id,
            email: "restricted@example.com".to_string(),
            email_verified: true,
            default_org_id: None,
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            disabled_at: None,
        })
    }

    async fn check_permission(&self, _ctx: &AuthCtx, permission: &str) -> Result<bool, AuthError> {
        Ok(self.permissions.contains(&permission.to_string()))
    }

    async fn jwks(&self) -> Result<Jwks, AuthError> {
        Ok(Jwks { keys: vec![] })
    }
}

fn create_restricted_test_state(
    pool: PgPool,
    schema: &str,
    permissions: Vec<String>,
) -> DataState<PgDataStore> {
    let store = Arc::new(PgDataStore::new(pool));
    let config = Arc::new(DataConfig {
        database_url: "postgres://test".to_string(),
        bind: "127.0.0.1:8080".parse().unwrap(),
        migrations_dir: None,
        run_migrations: false,
        user_schema: schema.to_string(),
        max_embed_depth: 5,
        max_limit: 1000,
        default_limit: 100,
        deployment: reactor_data::Deployment::Monolith,
        auth_url: None,
        internal_secret: None,
        auth_database_url: Some("postgres://test".to_string()),
        auth_data_key: Some("test_key_0000000000000000000000".to_string()),
        log: "debug".to_string(),
        metrics: false,
    });
    let auth: Arc<dyn AuthClient> = Arc::new(RestrictedMockAuthClient::new(permissions));
    DataState { config, store, auth }
}

#[tokio::test]
async fn test_policy_select_with_no_policies_allows_all() {
    // When no policies exist, all rows should be returned
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    // Use restricted auth client (no `*` permission)
    let state = create_restricted_test_state(pool, "app", vec!["data:read".to_string()]);
    let app = router(state);

    let (status, _headers, body) =
        make_request(app, Method::GET, "/data/v1/todos", None, vec![]).await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    // Should return all 3 rows since no policies are defined
    assert_eq!(rows.len(), 3);
}

#[tokio::test]
async fn test_superuser_bypass_with_star_permission() {
    // Users with `*` permission should bypass all policies
    let container = Postgres::default()
        .with_tag("15-alpine")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    setup_database(&pool)
        .await
        .expect("Failed to setup database");

    // Insert a restrictive policy (should be bypassed by superuser)
    sqlx::query(
        r#"
        INSERT INTO _reactor_data.policies (schema_name, table_name, name, scopes, using_ast, migration_name)
        VALUES ('app', 'todos', 'deny_all', '{select}', '{"kind":{"type":"Literal","literal_type":"Bool","value":false}}', 'test')
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to insert policy");

    // Use superuser auth client with `*` permission
    let state = create_test_state(pool, "app"); // This uses MockAuthClient with `*` permission
    let app = router(state);

    let (status, _headers, body) =
        make_request(app, Method::GET, "/data/v1/todos", None, vec![]).await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("Expected array response");
    // Should return all rows because `*` permission bypasses policies
    assert_eq!(rows.len(), 3);
}
