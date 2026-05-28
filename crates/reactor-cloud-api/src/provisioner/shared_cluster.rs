//! Shared cluster provider for Phase 4 deployments.
//!
//! This provider provisions tenant infrastructure on a shared multi-tenant cluster:
//! - Creates a dedicated database for each tenant (tenant_<ref>)
//! - Creates a dedicated database role with limited connections
//! - Runs capability migrations into that database
//! - Bootstraps vault keys
//! - Configures gateway routing with backend_kind='shared'
//!
//! # DB-per-tenant rationale
//!
//! We use database-per-tenant rather than schema-per-tenant because:
//! - Postgres handles ~10k DBs more gracefully than ~10k schemas in search_path
//! - Per-tenant role + connection limits are native (ALTER ROLE ... CONNECTION LIMIT)
//! - pg_dump/pg_basebackup per tenant is trivial
//! - Phase 5 migration to dedicated needs logical replication, which is per-database

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

use crate::bootstrap::{BootstrapConfig, SchemaBootstrap, VaultBootstrap};
use crate::error::{HealthError, ProvisionError, TeardownError};
use crate::provisioner::CloudProvider;
use crate::store::ProjectStore;
use crate::types::{BackendKind, ProjectHealth, ProjectSpec, ProvisionResult};
use reactor_core::{ProjectId, SecretValue, Vault};

/// Shared cluster provider for Phase 4.
///
/// Provisions tenants on a shared multi-tenant cluster with:
/// - Database-per-tenant isolation (CREATE DATABASE tenant_<ref>)
/// - Per-tenant Postgres roles with connection limits
/// - Shared vault with tenant-scoped paths
/// - Gateway routes with backend_kind='shared'
pub struct SharedClusterProvider {
    /// Connection pool to the control-plane Postgres (reactor_cloud schema).
    control_pool: PgPool,

    /// Connection pool to the shared Postgres superuser (for CREATE DATABASE).
    /// This must have superuser or CREATEDB privileges.
    admin_pool: PgPool,

    /// Vault for secrets management.
    vault: Arc<dyn Vault>,

    /// Project store.
    store: Arc<dyn ProjectStore>,

    /// Configuration.
    config: SharedClusterConfig,
}

/// Configuration for the shared cluster provider.
#[derive(Debug, Clone)]
pub struct SharedClusterConfig {
    /// Backend target for gateway routing (e.g., "rc-shared-1-server.internal:8000").
    pub backend_target: String,

    /// Base domain for project subdomains.
    pub base_domain: String,

    /// TLS mode for routes.
    pub tls_mode: String,

    /// Default connection limit per tenant database role.
    pub default_connection_limit: i32,

    /// Database collation for tenant databases.
    pub database_collation: String,

    /// Database encoding for tenant databases.
    pub database_encoding: String,

    /// Base Postgres URL used for both creating and connecting to tenant
    /// databases (e.g. `postgres://reactor:***@rc-super-shared-1-pg.internal:5432`).
    /// The provider connects as this superuser/admin role, then `tenant_db_url`
    /// substitutes the database name to migrate the freshly-created tenant DB.
    /// If `None`, tenant migration falls back to a localhost placeholder which
    /// will fail on real deployments — set this in `[cloud.shared_pool].shared_postgres_url`.
    pub shared_postgres_url: Option<String>,
}

impl Default for SharedClusterConfig {
    fn default() -> Self {
        Self {
            backend_target: "rc-shared-1-server.internal:8000".to_string(),
            base_domain: "reactor.cloud".to_string(),
            tls_mode: "wildcard".to_string(),
            default_connection_limit: 5,
            database_collation: "en_US.utf8".to_string(),
            database_encoding: "UTF8".to_string(),
            shared_postgres_url: None,
        }
    }
}

impl SharedClusterProvider {
    /// Create a new shared cluster provider.
    ///
    /// # Arguments
    /// * `control_pool` - Pool for control-plane operations (reactor_cloud schema)
    /// * `admin_pool` - Pool with CREATEDB privileges for tenant database operations
    /// * `vault` - Vault for secrets management
    /// * `store` - Project store
    /// * `config` - Provider configuration
    pub fn new(
        control_pool: PgPool,
        admin_pool: PgPool,
        vault: Arc<dyn Vault>,
        store: Arc<dyn ProjectStore>,
        config: SharedClusterConfig,
    ) -> Self {
        Self {
            control_pool,
            admin_pool,
            vault,
            store,
            config,
        }
    }

    /// Get the database name for a project ref.
    fn db_name(project_ref: &str) -> String {
        format!("tenant_{}", project_ref)
    }

    /// Get the database role name for a project ref.
    fn role_name(project_ref: &str) -> String {
        format!("tenant_{}", project_ref)
    }

    /// Get the hostname for a project ref.
    fn hostname(&self, project_ref: &str) -> String {
        format!("{}.{}", project_ref, self.config.base_domain)
    }

    /// Generate a secure random password for the tenant role.
    fn generate_password() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let bytes: [u8; 32] = rng.gen();
        hex::encode(bytes)
    }

    /// Create tenant database and role.
    ///
    /// Creates:
    /// 1. A database role with LOGIN and limited connections
    /// 2. A database owned by that role
    async fn create_tenant_database(
        &self,
        project_ref: &str,
        password: &str,
    ) -> Result<(), ProvisionError> {
        let db_name = Self::db_name(project_ref);
        let role_name = Self::role_name(project_ref);

        // Create role with password and connection limit
        // Note: We can't use placeholders for identifiers, so we validate the project_ref
        if !project_ref.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(ProvisionError::InvalidArgument(
                "project_ref contains invalid characters".to_string(),
            ));
        }

        debug!(role = %role_name, "creating tenant role");

        // Create the role
        let create_role_sql = format!(
            r#"CREATE ROLE "{}" WITH LOGIN PASSWORD '{}' CONNECTION LIMIT {}"#,
            role_name, password, self.config.default_connection_limit
        );

        sqlx::query(&create_role_sql)
            .execute(&self.admin_pool)
            .await
            .map_err(|e| ProvisionError::DatabaseSetup(format!("failed to create role: {}", e)))?;

        debug!(db = %db_name, owner = %role_name, "creating tenant database");

        // Create the database owned by the role
        let create_db_sql = format!(
            r#"CREATE DATABASE "{}" OWNER "{}" ENCODING '{}' LC_COLLATE '{}' LC_CTYPE '{}'"#,
            db_name, role_name,
            self.config.database_encoding,
            self.config.database_collation,
            self.config.database_collation
        );

        sqlx::query(&create_db_sql)
            .execute(&self.admin_pool)
            .await
            .map_err(|e| ProvisionError::DatabaseSetup(format!("failed to create database: {}", e)))?;

        // Grant CONNECT to the role (usually implicit with ownership, but explicit is safer)
        let grant_sql = format!(
            r#"GRANT CONNECT ON DATABASE "{}" TO "{}""#,
            db_name, role_name
        );

        sqlx::query(&grant_sql)
            .execute(&self.admin_pool)
            .await
            .map_err(|e| ProvisionError::DatabaseSetup(format!("failed to grant connect: {}", e)))?;

        Ok(())
    }

    /// Run capability migrations in the tenant database.
    async fn run_tenant_migrations(&self, project_ref: &str) -> Result<(), ProvisionError> {
        let db_name = Self::db_name(project_ref);

        // Connect to the tenant database
        let db_url = self.tenant_db_url(project_ref);

        let tenant_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .map_err(|e| ProvisionError::Migration(format!("failed to connect to tenant db: {}", e)))?;

        // Run migrations in the public schema (default for db-per-tenant)
        debug!(db = %db_name, "running capability migrations in tenant database");

        let bootstrap_config = BootstrapConfig::default();
        SchemaBootstrap::run_migrations(&tenant_pool, "public", &bootstrap_config)
            .await
            .map_err(|e| ProvisionError::Migration(e.to_string()))?;

        Ok(())
    }

    /// Build the connection URL for a tenant database.
    ///
    /// Parses `config.shared_postgres_url` (the admin/superuser connection
    /// string used to CREATE the tenant DB) and substitutes the database
    /// name. Migrations are run as the admin user — not the tenant role —
    /// because we just created the DB and need DDL privileges.
    fn tenant_db_url(&self, project_ref: &str) -> String {
        let db_name = Self::db_name(project_ref);

        if let Some(base) = &self.config.shared_postgres_url {
            return swap_db_name(base, &db_name);
        }

        // Fallback used only by tests/local dev when no shared_postgres_url
        // is configured. Real deployments must set this in config.
        format!("postgres://postgres@localhost:5432/{}", db_name)
    }

    /// Drop tenant database and role.
    async fn drop_tenant_database(&self, project_ref: &str) -> Result<(), TeardownError> {
        let db_name = Self::db_name(project_ref);
        let role_name = Self::role_name(project_ref);

        // Terminate existing connections to the database
        debug!(db = %db_name, "terminating connections to tenant database");

        let terminate_sql = format!(
            r#"SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}'"#,
            db_name
        );

        // Ignore errors - there may be no connections
        let _ = sqlx::query(&terminate_sql).execute(&self.admin_pool).await;

        // Drop the database
        debug!(db = %db_name, "dropping tenant database");

        let drop_db_sql = format!(r#"DROP DATABASE IF EXISTS "{}""#, db_name);

        sqlx::query(&drop_db_sql)
            .execute(&self.admin_pool)
            .await
            .map_err(|e| TeardownError::SchemaDrop(format!("failed to drop database: {}", e)))?;

        // Drop the role
        debug!(role = %role_name, "dropping tenant role");

        let drop_role_sql = format!(r#"DROP ROLE IF EXISTS "{}""#, role_name);

        sqlx::query(&drop_role_sql)
            .execute(&self.admin_pool)
            .await
            .map_err(|e| TeardownError::SchemaDrop(format!("failed to drop role: {}", e)))?;

        Ok(())
    }

    /// Check if a tenant database exists.
    async fn database_exists(&self, project_ref: &str) -> Result<bool, HealthError> {
        let db_name = Self::db_name(project_ref);

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)"
        )
        .bind(&db_name)
        .fetch_one(&self.admin_pool)
        .await
        .map_err(|e| HealthError::DatabaseConnectivity(e.to_string()))?;

        Ok(exists)
    }
}

#[async_trait]
impl CloudProvider for SharedClusterProvider {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Shared
    }

    #[instrument(skip(self), fields(project_id = %spec.project_id, project_ref = %spec.project_ref))]
    async fn provision(&self, spec: &ProjectSpec) -> Result<ProvisionResult, ProvisionError> {
        info!("provisioning project on shared cluster");

        let project_id = spec.project_id;
        let project_ref = spec.project_ref.as_str();

        // Step 1: Generate database password
        let db_password = Self::generate_password();

        // Step 2: Create tenant database and role
        debug!("creating tenant database and role");
        self.create_tenant_database(project_ref, &db_password).await?;

        // Step 3: Run capability migrations
        debug!("running capability migrations");
        self.run_tenant_migrations(project_ref).await?;

        // Step 4: Bootstrap vault keys
        debug!("bootstrapping vault keys");
        let vault_result = VaultBootstrap::bootstrap(&self.vault, &project_id)
            .await
            .map_err(|e| ProvisionError::VaultBootstrap(e.to_string()))?;

        // Step 5: Store database password in vault
        debug!("storing database password in vault");
        self.vault
            .put_secret(
                &project_id,
                "db/password",
                SecretValue::new(db_password.clone()),
            )
            .await
            .map_err(|e| ProvisionError::VaultBootstrap(format!("failed to store db password: {}", e)))?;

        // Step 6: Generate API keys (JWTs)
        debug!("generating API keys");
        let anon_key = VaultBootstrap::generate_anon_jwt(
            &self.vault,
            &project_id,
            project_ref,
        )
        .await
        .map_err(|e| ProvisionError::VaultBootstrap(format!("anon JWT: {}", e)))?;

        let service_key = VaultBootstrap::generate_service_jwt(
            &self.vault,
            &project_id,
            project_ref,
        )
        .await
        .map_err(|e| ProvisionError::VaultBootstrap(format!("service JWT: {}", e)))?;

        // Step 7: Create gateway route with backend_kind='shared'
        debug!("creating gateway route");
        let hostname = self.hostname(project_ref);
        self.store
            .create_route(
                &hostname,
                &project_id,
                project_ref,
                "shared", // backend_kind
                &self.config.backend_target,
                &self.config.tls_mode,
            )
            .await
            .map_err(|e| ProvisionError::RouteCreation(e.to_string()))?;

        info!(
            hostname = %hostname,
            transit_keys = vault_result.transit_keys_created,
            backend_kind = "shared",
            "project provisioned successfully on shared cluster"
        );

        Ok(ProvisionResult {
            backend_target: self.config.backend_target.clone(),
            anon_key,
            service_key,
        })
    }

    #[instrument(skip(self), fields(project_id = %project_id))]
    async fn teardown(&self, project_id: &ProjectId) -> Result<(), TeardownError> {
        info!("tearing down project from shared cluster");

        // Get project info first
        let project = self
            .store
            .get_project_by_id(project_id)
            .await
            .map_err(|e| TeardownError::Database(e.to_string()))?
            .ok_or_else(|| TeardownError::Database("project not found".to_string()))?;

        let project_ref = &project.project_ref;
        let hostname = self.hostname(project_ref);

        // Step 1: Remove gateway route
        debug!(hostname = %hostname, "removing gateway route");
        if let Err(e) = self.store.delete_route(&hostname).await {
            error!(error = %e, "failed to delete route (continuing)");
        }

        // Step 2: Delete vault secrets and keys
        debug!("cleaning up vault secrets");
        if let Err(e) = VaultBootstrap::cleanup(&self.vault, project_id).await {
            error!(error = %e, "failed to cleanup vault (continuing)");
        }

        // Step 3: Drop tenant database and role
        debug!("dropping tenant database and role");
        self.drop_tenant_database(project_ref)
            .await
            .map_err(|e| {
                error!(error = %e, "failed to drop tenant database");
                e
            })?;

        info!("project teardown complete from shared cluster");
        Ok(())
    }

    #[instrument(skip(self), fields(project_id = %project_id))]
    async fn health(&self, project_id: &ProjectId) -> Result<ProjectHealth, HealthError> {
        // Get project info
        let project = self
            .store
            .get_project_by_id(project_id)
            .await
            .map_err(|e| HealthError::DatabaseConnectivity(e.to_string()))?
            .ok_or(HealthError::SchemaNotFound)?;

        let hostname = self.hostname(&project.project_ref);

        // Check database exists
        let db_exists = self.database_exists(&project.project_ref).await?;

        // Check vault accessible
        let vault_accessible = self.vault.is_healthy().await;

        // Check route exists
        let route_configured = self
            .store
            .get_route(&hostname)
            .await
            .map(|r| r.is_some())
            .unwrap_or(false);

        let healthy = db_exists && vault_accessible && route_configured;

        Ok(ProjectHealth {
            healthy,
            schema_exists: db_exists, // Using schema_exists field for db_exists
            vault_accessible,
            route_configured,
            error: if healthy {
                None
            } else {
                Some(format!(
                    "db={}, vault={}, route={}",
                    db_exists, vault_accessible, route_configured
                ))
            },
        })
    }
}

/// Replace the database name in a Postgres URL.
///
/// Handles `postgres://user:pass@host:port/dbname?params` and
/// `postgres://user:pass@host:port` (no path). Preserves query string.
fn swap_db_name(base_url: &str, new_db: &str) -> String {
    // Split off the optional query string.
    let (without_query, query) = match base_url.find('?') {
        Some(i) => (&base_url[..i], Some(&base_url[i..])),
        None => (base_url, None),
    };

    // Find the path separator after the host (i.e. the third '/').
    // postgres://user:pass@host:port/dbname
    //  ^scheme  ^         ^
    // We skip past `://` and then look for the next `/`.
    let body_start = without_query.find("://").map(|i| i + 3).unwrap_or(0);
    let after_scheme = &without_query[body_start..];

    let new_url = match after_scheme.find('/') {
        Some(rel_path_start) => {
            let host_part = &without_query[..body_start + rel_path_start];
            format!("{}/{}", host_part, new_db)
        }
        None => format!("{}/{}", without_query, new_db),
    };

    match query {
        Some(q) => format!("{}{}", new_url, q),
        None => new_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_db_name() {
        assert_eq!(
            swap_db_name("postgres://u:p@h:5432/control", "tenant_x"),
            "postgres://u:p@h:5432/tenant_x"
        );
        assert_eq!(
            swap_db_name("postgres://u:p@h:5432", "tenant_x"),
            "postgres://u:p@h:5432/tenant_x"
        );
        assert_eq!(
            swap_db_name("postgres://u:p@h:5432/control?sslmode=disable", "tenant_x"),
            "postgres://u:p@h:5432/tenant_x?sslmode=disable"
        );
    }

    #[test]
    fn test_db_name() {
        assert_eq!(
            SharedClusterProvider::db_name("myproject12345678ab"),
            "tenant_myproject12345678ab"
        );
    }

    #[test]
    fn test_role_name() {
        assert_eq!(
            SharedClusterProvider::role_name("myproject12345678ab"),
            "tenant_myproject12345678ab"
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = SharedClusterConfig::default();
        assert_eq!(config.backend_target, "rc-shared-1-server.internal:8000");
        assert_eq!(config.base_domain, "reactor.cloud");
        assert_eq!(config.default_connection_limit, 5);
    }

    #[test]
    fn test_generate_password() {
        let pw1 = SharedClusterProvider::generate_password();
        let pw2 = SharedClusterProvider::generate_password();

        // Passwords should be 64 hex chars (32 bytes)
        assert_eq!(pw1.len(), 64);
        assert_eq!(pw2.len(), 64);

        // Should be unique
        assert_ne!(pw1, pw2);

        // Should be valid hex
        assert!(pw1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
