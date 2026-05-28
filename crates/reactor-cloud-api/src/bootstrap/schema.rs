//! Schema bootstrapping for tenant databases.
//!
//! Creates tenant schemas and runs capability migrations with proper isolation.

use sqlx::{Executor, PgPool};
use thiserror::Error;
use tracing::{debug, info};

/// Error type for schema bootstrap operations.
#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("schema already exists: {0}")]
    AlreadyExists(String),

    #[error("schema creation failed: {0}")]
    CreationFailed(String),

    #[error("migration failed: {0}")]
    MigrationFailed(String),

    #[error("schema drop failed: {0}")]
    DropFailed(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Configuration for schema bootstrap operations.
#[derive(Debug, Clone)]
pub struct BootstrapConfig {
    /// Whether to run auth capability migrations.
    pub enable_auth: bool,
    /// Whether to run data capability migrations.
    pub enable_data: bool,
    /// Whether to run storage capability migrations.
    pub enable_storage: bool,
    /// Whether to run functions capability migrations.
    pub enable_functions: bool,
    /// Whether to run jobs capability migrations.
    pub enable_jobs: bool,
    /// Whether to run sites capability migrations.
    pub enable_sites: bool,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            enable_auth: true,
            enable_data: true,
            enable_storage: true,
            enable_functions: true,
            enable_jobs: true,
            enable_sites: true,
        }
    }
}

/// Schema bootstrap operations.
pub struct SchemaBootstrap;

impl SchemaBootstrap {
    /// Check if a schema exists.
    pub async fn schema_exists(pool: &PgPool, schema: &str) -> Result<bool, SchemaError> {
        let result: (bool,) = sqlx::query_as(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.schemata 
                WHERE schema_name = $1
            )
            "#,
        )
        .bind(schema)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    /// Create a new tenant schema.
    ///
    /// The schema name should be `tenant_<project_ref>`.
    pub async fn create_schema(pool: &PgPool, schema: &str) -> Result<(), SchemaError> {
        // Validate schema name
        if !schema.starts_with("tenant_") {
            return Err(SchemaError::CreationFailed(format!(
                "invalid schema name: must start with 'tenant_', got '{}'",
                schema
            )));
        }

        // Check for SQL injection (schema name should be alphanumeric + underscore)
        if !schema.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(SchemaError::CreationFailed(format!(
                "invalid schema name: contains invalid characters: '{}'",
                schema
            )));
        }

        debug!(schema = %schema, "creating tenant schema");

        // Create the schema
        let sql = format!("CREATE SCHEMA IF NOT EXISTS {}", schema);
        pool.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::CreationFailed(e.to_string()))?;

        // Create migrations tracking table for this tenant
        let sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}._migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )
            "#,
            schema
        );
        pool.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::CreationFailed(e.to_string()))?;

        info!(schema = %schema, "tenant schema created");
        Ok(())
    }

    /// Run capability migrations for a tenant schema.
    ///
    /// This sets `search_path` to the tenant schema before running each
    /// capability's migrations, ensuring tables are created in the right schema.
    pub async fn run_migrations(
        pool: &PgPool,
        schema: &str,
        config: &BootstrapConfig,
    ) -> Result<(), SchemaError> {
        debug!(schema = %schema, "running tenant capability migrations");

        // Pre-create shared extensions in the public schema. Auth migrations
        // use CITEXT for case-insensitive emails; without this the migration
        // fails with `type "citext" does not exist`.
        pool.execute("CREATE EXTENSION IF NOT EXISTS citext WITH SCHEMA public")
            .await
            .map_err(|e| SchemaError::MigrationFailed(format!("create citext: {}", e)))?;

        // Ensure the `_migrations` tracking table exists in the target schema.
        // `create_schema` creates this for prefixed tenant schemas, but the
        // db-per-tenant flow passes `public` and skips create_schema.
        let migrations_table_sql = format!(
            r#"CREATE TABLE IF NOT EXISTS {}._migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )"#,
            schema
        );
        pool.execute(migrations_table_sql.as_str())
            .await
            .map_err(|e| SchemaError::MigrationFailed(format!("create _migrations: {}", e)))?;

        // Acquire a connection and set search_path
        let mut conn = pool.acquire().await?;

        // Set search_path to tenant schema (tables created here) + public (for extensions)
        let sql = format!("SET search_path TO {}, public", schema);
        conn.execute(sql.as_str()).await?;

        // Run migrations for each enabled capability
        // The migrations will use the search_path, creating tables in the tenant schema

        if config.enable_auth {
            Self::run_auth_migrations(&mut conn, schema).await?;
        }

        if config.enable_data {
            Self::run_data_migrations(&mut conn, schema).await?;
        }

        if config.enable_storage {
            Self::run_storage_migrations(&mut conn, schema).await?;
        }

        if config.enable_functions {
            Self::run_functions_migrations(&mut conn, schema).await?;
        }

        if config.enable_jobs {
            Self::run_jobs_migrations(&mut conn, schema).await?;
        }

        if config.enable_sites {
            Self::run_sites_migrations(&mut conn, schema).await?;
        }

        info!(schema = %schema, "tenant migrations complete");
        Ok(())
    }

    /// Run auth capability migrations.
    async fn run_auth_migrations(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
        schema: &str,
    ) -> Result<(), SchemaError> {
        debug!(schema = %schema, "running auth migrations");

        // Create auth tables in the tenant schema
        // These are simplified versions - full migrations would come from reactor-auth
        let sql = format!(
            r#"
            -- Users table
            CREATE TABLE IF NOT EXISTS {schema}.auth_users (
                id UUID PRIMARY KEY,
                email CITEXT NOT NULL UNIQUE,
                password_hash TEXT,
                display_name TEXT,
                avatar_url TEXT,
                email_verified_at TIMESTAMPTZ,
                last_sign_in_at TIMESTAMPTZ,
                metadata JSONB NOT NULL DEFAULT '{{}}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            -- Sessions table
            CREATE TABLE IF NOT EXISTS {schema}.auth_sessions (
                id UUID PRIMARY KEY,
                user_id UUID NOT NULL REFERENCES {schema}.auth_users(id) ON DELETE CASCADE,
                refresh_token_hash TEXT NOT NULL,
                user_agent TEXT,
                ip_address INET,
                expires_at TIMESTAMPTZ NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            CREATE INDEX IF NOT EXISTS auth_sessions_user_id_idx ON {schema}.auth_sessions(user_id);
            CREATE INDEX IF NOT EXISTS auth_sessions_expires_idx ON {schema}.auth_sessions(expires_at);

            -- Track migration
            INSERT INTO {schema}._migrations (version, description)
            VALUES (1, 'auth_base_tables')
            ON CONFLICT (version) DO NOTHING;
            "#,
            schema = schema
        );

        conn.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::MigrationFailed(format!("auth: {}", e)))?;

        Ok(())
    }

    /// Run data capability migrations.
    async fn run_data_migrations(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
        schema: &str,
    ) -> Result<(), SchemaError> {
        debug!(schema = %schema, "running data migrations");

        // Create data tables (internal tracking, not user tables)
        let sql = format!(
            r#"
            -- User migrations tracking
            CREATE TABLE IF NOT EXISTS {schema}.data_migrations (
                id UUID PRIMARY KEY,
                version TEXT NOT NULL,
                name TEXT NOT NULL,
                sql_up TEXT NOT NULL,
                sql_down TEXT,
                applied_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            CREATE INDEX IF NOT EXISTS data_migrations_version_idx ON {schema}.data_migrations(version);

            -- Track migration
            INSERT INTO {schema}._migrations (version, description)
            VALUES (2, 'data_base_tables')
            ON CONFLICT (version) DO NOTHING;
            "#,
            schema = schema
        );

        conn.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::MigrationFailed(format!("data: {}", e)))?;

        Ok(())
    }

    /// Run storage capability migrations.
    async fn run_storage_migrations(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
        schema: &str,
    ) -> Result<(), SchemaError> {
        debug!(schema = %schema, "running storage migrations");

        let sql = format!(
            r#"
            -- Buckets table
            CREATE TABLE IF NOT EXISTS {schema}.storage_buckets (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                public BOOLEAN NOT NULL DEFAULT false,
                file_size_limit BIGINT,
                allowed_mime_types TEXT[],
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            -- Objects table
            CREATE TABLE IF NOT EXISTS {schema}.storage_objects (
                id UUID PRIMARY KEY,
                bucket_id TEXT NOT NULL REFERENCES {schema}.storage_buckets(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                owner_id UUID,
                content_type TEXT,
                size BIGINT NOT NULL DEFAULT 0,
                etag TEXT,
                metadata JSONB NOT NULL DEFAULT '{{}}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                UNIQUE (bucket_id, name)
            );

            CREATE INDEX IF NOT EXISTS storage_objects_bucket_idx ON {schema}.storage_objects(bucket_id);
            CREATE INDEX IF NOT EXISTS storage_objects_owner_idx ON {schema}.storage_objects(owner_id);

            -- Track migration
            INSERT INTO {schema}._migrations (version, description)
            VALUES (3, 'storage_base_tables')
            ON CONFLICT (version) DO NOTHING;
            "#,
            schema = schema
        );

        conn.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::MigrationFailed(format!("storage: {}", e)))?;

        Ok(())
    }

    /// Run functions capability migrations.
    async fn run_functions_migrations(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
        schema: &str,
    ) -> Result<(), SchemaError> {
        debug!(schema = %schema, "running functions migrations");

        let sql = format!(
            r#"
            -- Functions table
            CREATE TABLE IF NOT EXISTS {schema}.functions (
                id UUID PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                runtime TEXT NOT NULL DEFAULT 'wasm',
                entry_point TEXT,
                env_vars JSONB NOT NULL DEFAULT '{{}}',
                current_deployment_id UUID,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            -- Function deployments
            CREATE TABLE IF NOT EXISTS {schema}.function_deployments (
                id UUID PRIMARY KEY,
                function_id UUID NOT NULL REFERENCES {schema}.functions(id) ON DELETE CASCADE,
                bundle_hash TEXT NOT NULL,
                bundle_size BIGINT NOT NULL,
                storage_path TEXT NOT NULL,
                deployed_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            CREATE INDEX IF NOT EXISTS function_deployments_fn_idx ON {schema}.function_deployments(function_id);

            -- Track migration
            INSERT INTO {schema}._migrations (version, description)
            VALUES (4, 'functions_base_tables')
            ON CONFLICT (version) DO NOTHING;
            "#,
            schema = schema
        );

        conn.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::MigrationFailed(format!("functions: {}", e)))?;

        Ok(())
    }

    /// Run jobs capability migrations.
    async fn run_jobs_migrations(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
        schema: &str,
    ) -> Result<(), SchemaError> {
        debug!(schema = %schema, "running jobs migrations");

        let sql = format!(
            r#"
            -- Jobs table
            CREATE TABLE IF NOT EXISTS {schema}.jobs (
                id UUID PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                schedule TEXT,
                function_name TEXT NOT NULL,
                payload JSONB,
                enabled BOOLEAN NOT NULL DEFAULT true,
                retry_limit INT NOT NULL DEFAULT 3,
                timeout_ms BIGINT NOT NULL DEFAULT 300000,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            -- Job runs
            CREATE TABLE IF NOT EXISTS {schema}.job_runs (
                id UUID PRIMARY KEY,
                job_id UUID NOT NULL REFERENCES {schema}.jobs(id) ON DELETE CASCADE,
                status TEXT NOT NULL DEFAULT 'pending',
                started_at TIMESTAMPTZ,
                completed_at TIMESTAMPTZ,
                result JSONB,
                error TEXT,
                attempt INT NOT NULL DEFAULT 1,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            CREATE INDEX IF NOT EXISTS job_runs_job_idx ON {schema}.job_runs(job_id);
            CREATE INDEX IF NOT EXISTS job_runs_status_idx ON {schema}.job_runs(status);

            -- Track migration
            INSERT INTO {schema}._migrations (version, description)
            VALUES (5, 'jobs_base_tables')
            ON CONFLICT (version) DO NOTHING;
            "#,
            schema = schema
        );

        conn.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::MigrationFailed(format!("jobs: {}", e)))?;

        Ok(())
    }

    /// Run sites capability migrations.
    async fn run_sites_migrations(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
        schema: &str,
    ) -> Result<(), SchemaError> {
        debug!(schema = %schema, "running sites migrations");

        let sql = format!(
            r#"
            -- Sites table
            CREATE TABLE IF NOT EXISTS {schema}.sites (
                id UUID PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                current_deployment_id UUID,
                default_domain TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            -- Site deployments
            CREATE TABLE IF NOT EXISTS {schema}.site_deployments (
                id UUID PRIMARY KEY,
                site_id UUID NOT NULL REFERENCES {schema}.sites(id) ON DELETE CASCADE,
                bundle_hash TEXT NOT NULL,
                bundle_size BIGINT NOT NULL,
                storage_path TEXT NOT NULL,
                deployed_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            CREATE INDEX IF NOT EXISTS site_deployments_site_idx ON {schema}.site_deployments(site_id);

            -- Site domains
            CREATE TABLE IF NOT EXISTS {schema}.site_domains (
                id UUID PRIMARY KEY,
                site_id UUID NOT NULL REFERENCES {schema}.sites(id) ON DELETE CASCADE,
                domain TEXT NOT NULL UNIQUE,
                verified_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );

            CREATE INDEX IF NOT EXISTS site_domains_site_idx ON {schema}.site_domains(site_id);

            -- Track migration
            INSERT INTO {schema}._migrations (version, description)
            VALUES (6, 'sites_base_tables')
            ON CONFLICT (version) DO NOTHING;
            "#,
            schema = schema
        );

        conn.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::MigrationFailed(format!("sites: {}", e)))?;

        Ok(())
    }

    /// Drop a tenant schema and all its contents.
    pub async fn drop_schema(pool: &PgPool, schema: &str) -> Result<(), SchemaError> {
        // Validate schema name
        if !schema.starts_with("tenant_") {
            return Err(SchemaError::DropFailed(format!(
                "refusing to drop non-tenant schema: '{}'",
                schema
            )));
        }

        // Check for SQL injection
        if !schema.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(SchemaError::DropFailed(format!(
                "invalid schema name: '{}'",
                schema
            )));
        }

        debug!(schema = %schema, "dropping tenant schema");

        let sql = format!("DROP SCHEMA IF EXISTS {} CASCADE", schema);
        pool.execute(sql.as_str())
            .await
            .map_err(|e| SchemaError::DropFailed(e.to_string()))?;

        info!(schema = %schema, "tenant schema dropped");
        Ok(())
    }
}
