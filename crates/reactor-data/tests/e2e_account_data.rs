//! End-to-end integration test with real auth + data routers.
//!
//! This test uses the InProcess topology to exercise the full flow:
//! JWT issue → JWT verify → resolve_ctx → policy → SQL
//!
//! Scenario:
//! 1. Start Postgres testcontainer
//! 2. Apply auth and data migrations
//! 3. Sign up a user via /auth/v1/signup
//! 4. Create an org via /auth/v1/orgs
//! 5. Use the issued JWT to POST and GET /data/v1/todos
//! 6. Verify audit events are recorded

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reactor_auth::{client::InProcessAuthClient, AuthConfig, AuthState};
use reactor_core::auth::AuthClient;
use reactor_data::{router as data_router, DataConfig, DataState, Deployment, PgDataStore};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tower::ServiceExt;
use url::Url;

/// Generate a valid 32-byte base64-encoded key for testing.
fn test_data_key() -> String {
    BASE64.encode([0u8; 32])
}

/// Create an AuthConfig for testing.
fn test_auth_config(database_url: &str) -> AuthConfig {
    AuthConfig {
        database_url: database_url.to_string(),
        bind: "127.0.0.1:0".parse().unwrap(),
        data_key: test_data_key(),
        jwt_issuer: "reactor-auth".to_string(),
        jwt_audience: "reactor".to_string(),
        access_ttl_secs: 3600,
        refresh_ttl_secs: 86400,
        internal_secret: Some("test-secret".to_string()),
        public_url: Url::parse("http://localhost:8001").unwrap(),
        smtp: None,
        log: "warn".to_string(),
        metrics: false,
    }
}

/// Create a DataConfig for testing.
fn test_data_config(database_url: &str) -> DataConfig {
    DataConfig {
        database_url: database_url.to_string(),
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

/// Set up the test database with auth schema, data schema, and app.todos table.
async fn setup_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Run auth migrations
    reactor_auth::migrator()
        .run(pool)
        .await
        .expect("failed to run auth migrations");

    // Create _reactor_data schema and tables (simulating data migrations)
    sqlx::query("CREATE SCHEMA IF NOT EXISTS _reactor_data")
        .execute(pool)
        .await?;

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

    // Create user schema and todos table
    sqlx::query("CREATE SCHEMA IF NOT EXISTS app")
        .execute(pool)
        .await?;

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

/// Helper to make requests to the auth router.
async fn auth_request(
    app: &axum::Router,
    method: Method,
    path: &str,
    body: Option<Value>,
    token: Option<&str>,
    headers: Option<Vec<(&str, &str)>>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");

    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", t));
    }

    if let Some(hdrs) = headers {
        for (k, v) in hdrs {
            builder = builder.header(k, v);
        }
    }

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

/// Helper to make requests to the data router.
async fn data_request(
    app: &axum::Router,
    method: Method,
    path: &str,
    body: Option<Value>,
    token: &str,
    org_header: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json");

    if let Some(org) = org_header {
        builder = builder.header("x-reactor-org", org);
    }

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

/// Full end-to-end test: signup → create org → CRUD todos → verify audit.
#[tokio::test]
async fn test_e2e_signup_org_data_flow() {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("reactor_data=debug,reactor_auth=debug")
        .with_test_writer()
        .try_init();

    // Start Postgres container
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("failed to start postgres");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let pool = PgPool::connect(&db_url).await.expect("failed to connect");

    // Set up database with auth + data schemas
    setup_database(&pool).await.expect("failed to setup database");

    // Create AuthState with real auth service
    let auth_config = test_auth_config(&db_url);
    let auth_state = AuthState::from_pool(pool.clone(), auth_config)
        .expect("failed to create auth state");

    // Bootstrap signing key
    auth_state
        .keyring
        .ensure_active_key()
        .await
        .expect("failed to bootstrap signing key");

    // Build auth router
    let auth_app = reactor_auth::router(auth_state.clone());

    // Build data router with InProcessAuthClient
    let auth_client: Arc<dyn AuthClient> = Arc::new(InProcessAuthClient::new(auth_state.service.clone()));
    let data_store = Arc::new(PgDataStore::new(pool.clone()));
    let data_config = test_data_config(&db_url);
    let data_state = DataState::new(data_store, auth_client, Arc::new(data_config));
    let data_app = data_router(data_state);

    // ===== Step 1: Sign up a user =====
    let (status, body) = auth_request(
        &auth_app,
        Method::POST,
        "/auth/v1/signup",
        Some(json!({
            "email": "alice@test.local",
            "password": "hunter22hunter22"
        })),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "Signup failed: {:?}", body);
    let access_token = body["session"]["access_token"]
        .as_str()
        .expect("missing access_token");
    let refresh_token = body["session"]["refresh_token"]
        .as_str()
        .expect("missing refresh_token");

    // ===== Step 2: Create an organization =====
    let (status, body) = auth_request(
        &auth_app,
        Method::POST,
        "/auth/v1/orgs",
        Some(json!({
            "slug": "acme",
            "name": "Acme Corp"
        })),
        Some(access_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "Org creation failed: {:?}", body);
    let org_id = body["id"].as_str().expect("missing org id");

    // ===== Step 2b: Refresh the token to get updated claims with org membership =====
    let (status, body) = auth_request(
        &auth_app,
        Method::POST,
        "/auth/v1/token?grant_type=refresh_token",
        Some(json!({
            "refresh_token": refresh_token
        })),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "Token refresh failed: {:?}", body);
    let access_token = body["access_token"]
        .as_str()
        .expect("missing access_token after refresh");

    // ===== Step 3: Verify permissions (owner should have * permission) =====
    let (status, body) = auth_request(
        &auth_app,
        Method::GET,
        "/auth/v1/permissions",
        None,
        Some(access_token),
        Some(vec![("x-reactor-org", "acme")]),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "Get permissions failed: {:?}", body);
    let permissions = body["permissions"].as_array().expect("missing permissions");
    assert!(
        permissions.iter().any(|p| p.as_str() == Some("*")),
        "Owner should have * permission, got: {:?}",
        permissions
    );

    // ===== Step 4: Create a todo via the data API =====
    let (status, body) = data_request(
        &data_app,
        Method::POST,
        "/data/v1/todos",
        Some(json!({
            "org_id": org_id,
            "title": "My first task"
        })),
        access_token,
        Some("acme"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "Todo insert failed: {:?}", body);

    // ===== Step 5: Read todos via the data API =====
    let (status, body) = data_request(
        &data_app,
        Method::GET,
        "/data/v1/todos",
        None,
        access_token,
        Some("acme"),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "Todo read failed: {:?}", body);
    let rows = body.as_array().expect("expected array");
    assert_eq!(rows.len(), 1, "Expected 1 todo");
    assert_eq!(rows[0]["title"], "My first task");
    assert_eq!(rows[0]["done"], false);

    // ===== Step 6: Verify audit event was recorded =====
    let audit_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM _reactor_data.audit_events WHERE event_type = 'rows.insert'",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to query audit events");

    assert_eq!(audit_count.0, 1, "Expected 1 audit event for insert");

    // ===== Step 7: Update the todo =====
    let (status, _body) = data_request(
        &data_app,
        Method::PATCH,
        "/data/v1/todos?title=eq.My%20first%20task",
        Some(json!({
            "done": true
        })),
        access_token,
        Some("acme"),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT, "Todo update failed");

    // Verify the update
    let (status, body) = data_request(
        &data_app,
        Method::GET,
        "/data/v1/todos",
        None,
        access_token,
        Some("acme"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("expected array");
    assert_eq!(rows[0]["done"], true);

    // ===== Step 8: Delete the todo =====
    let (status, _body) = data_request(
        &data_app,
        Method::DELETE,
        "/data/v1/todos?title=eq.My%20first%20task",
        None,
        access_token,
        Some("acme"),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT, "Todo delete failed");

    // Verify deletion
    let (status, body) = data_request(
        &data_app,
        Method::GET,
        "/data/v1/todos",
        None,
        access_token,
        Some("acme"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("expected array");
    assert_eq!(rows.len(), 0, "Expected 0 todos after delete");

    // Cleanup
    pool.close().await;
}

/// Test that health endpoints work without auth.
#[tokio::test]
async fn test_health_endpoints_no_auth() {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("failed to start postgres");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let pool = PgPool::connect(&db_url).await.expect("failed to connect");
    setup_database(&pool).await.expect("failed to setup database");

    // Auth health
    let auth_config = test_auth_config(&db_url);
    let auth_state = AuthState::from_pool(pool.clone(), auth_config)
        .expect("failed to create auth state");
    auth_state.keyring.ensure_active_key().await.unwrap();
    let auth_app = reactor_auth::router(auth_state.clone());

    let (status, body) = auth_request(&auth_app, Method::GET, "/auth/v1/health", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    // Data health
    let auth_client: Arc<dyn AuthClient> = Arc::new(InProcessAuthClient::new(auth_state.service.clone()));
    let data_store = Arc::new(PgDataStore::new(pool.clone()));
    let data_config = test_data_config(&db_url);
    let data_state = DataState::new(data_store, auth_client, Arc::new(data_config));
    let data_app = data_router(data_state);

    let request = Request::builder()
        .method(Method::GET)
        .uri("/data/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = data_app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    pool.close().await;
}

/// Test JWKS endpoint returns valid keys.
#[tokio::test]
async fn test_jwks_endpoint() {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("failed to start postgres");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let pool = PgPool::connect(&db_url).await.expect("failed to connect");
    setup_database(&pool).await.expect("failed to setup database");

    let auth_config = test_auth_config(&db_url);
    let auth_state = AuthState::from_pool(pool.clone(), auth_config)
        .expect("failed to create auth state");
    auth_state.keyring.ensure_active_key().await.unwrap();
    let auth_app = reactor_auth::router(auth_state);

    let (status, body) = auth_request(&auth_app, Method::GET, "/auth/v1/keys", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    
    let keys = body["keys"].as_array().expect("missing keys array");
    assert!(!keys.is_empty(), "JWKS should have at least one key");
    assert_eq!(keys[0]["kty"], "RSA");
    assert_eq!(keys[0]["alg"], "RS256");

    pool.close().await;
}
