//! PostgreSQL-backed routing store.
//!
//! This module provides storage for routes and custom domains using PostgreSQL,
//! with LISTEN/NOTIFY support for real-time updates.

use crate::error::{GatewayError, GatewayResult};
use crate::on_demand_tls::DomainVerifier;
use crate::routing::{
    BackendKind, BackendTarget, CertStatus, CustomDomain, Route, RoutingTable, TlsMode,
};
use crate::sync::RoutingTableLoader;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reactor_core::{ProjectId, ProjectRef};
use sqlx::postgres::{PgListener, PgPool};
use sqlx::FromRow;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// PostgreSQL routing store configuration.
#[derive(Debug, Clone)]
pub struct PgRoutingStoreConfig {
    /// Notification channel name.
    pub channel: String,
}

impl Default for PgRoutingStoreConfig {
    fn default() -> Self {
        Self {
            channel: "reactor_gateway_routes".to_string(),
        }
    }
}

/// PostgreSQL-backed routing store.
pub struct PgRoutingStore {
    pool: PgPool,
    config: PgRoutingStoreConfig,
}

impl PgRoutingStore {
    /// Create a new store with the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            config: PgRoutingStoreConfig::default(),
        }
    }

    /// Create a new store with custom configuration.
    pub fn with_config(pool: PgPool, config: PgRoutingStoreConfig) -> Self {
        Self { pool, config }
    }

    /// Get the database pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Load all routes from the database.
    pub async fn load_routes(&self) -> GatewayResult<Vec<Route>> {
        #[derive(FromRow)]
        struct RouteRow {
            host: String,
            project_id: uuid::Uuid,
            project_ref: String,
            backend_kind: String,
            backend_target: String,
            tls_mode: String,
            enabled: bool,
            updated_at: DateTime<Utc>,
        }

        let rows: Vec<RouteRow> = sqlx::query_as(
            r#"
            SELECT host, project_id, project_ref, backend_kind, backend_target, tls_mode, enabled, updated_at
            FROM reactor_gateway.routes
            WHERE enabled = true
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let routes = rows
            .into_iter()
            .filter_map(|row| {
                let backend_kind = BackendKind::from_str(&row.backend_kind).ok()?;
                let tls_mode = TlsMode::from_str(&row.tls_mode).ok()?;
                let project_id = ProjectId::from(row.project_id);
                let project_ref = ProjectRef::from_str(&row.project_ref).ok()?;

                Some(Route {
                    host: row.host,
                    project_id,
                    project_ref,
                    backend_kind,
                    backend_target: BackendTarget::new(row.backend_target),
                    tls_mode,
                    enabled: row.enabled,
                    updated_at: row.updated_at,
                })
            })
            .collect();

        Ok(routes)
    }

    /// Get a single route by host.
    pub async fn get_route(&self, host: &str) -> GatewayResult<Option<Route>> {
        #[derive(FromRow)]
        struct RouteRow {
            host: String,
            project_id: uuid::Uuid,
            project_ref: String,
            backend_kind: String,
            backend_target: String,
            tls_mode: String,
            enabled: bool,
            updated_at: DateTime<Utc>,
        }

        let row: Option<RouteRow> = sqlx::query_as(
            r#"
            SELECT host, project_id, project_ref, backend_kind, backend_target, tls_mode, enabled, updated_at
            FROM reactor_gateway.routes
            WHERE host = $1
            "#,
        )
        .bind(host)
        .fetch_optional(&self.pool)
        .await?;

        let route = row.and_then(|row| {
            let backend_kind = BackendKind::from_str(&row.backend_kind).ok()?;
            let tls_mode = TlsMode::from_str(&row.tls_mode).ok()?;
            let project_id = ProjectId::from(row.project_id);
            let project_ref = ProjectRef::from_str(&row.project_ref).ok()?;

            Some(Route {
                host: row.host,
                project_id,
                project_ref,
                backend_kind,
                backend_target: BackendTarget::new(row.backend_target),
                tls_mode,
                enabled: row.enabled,
                updated_at: row.updated_at,
            })
        });

        Ok(route)
    }

    /// Insert or update a route.
    pub async fn upsert_route(&self, route: &Route) -> GatewayResult<()> {
        sqlx::query(
            r#"
            INSERT INTO reactor_gateway.routes (host, project_id, project_ref, backend_kind, backend_target, tls_mode, enabled, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (host) DO UPDATE SET
                project_id = EXCLUDED.project_id,
                project_ref = EXCLUDED.project_ref,
                backend_kind = EXCLUDED.backend_kind,
                backend_target = EXCLUDED.backend_target,
                tls_mode = EXCLUDED.tls_mode,
                enabled = EXCLUDED.enabled,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&route.host)
        .bind(route.project_id.as_uuid())
        .bind(route.project_ref.as_str())
        .bind(route.backend_kind.to_string())
        .bind(&route.backend_target.address)
        .bind(route.tls_mode.to_string())
        .bind(route.enabled)
        .bind(route.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a route.
    pub async fn delete_route(&self, host: &str) -> GatewayResult<()> {
        sqlx::query("DELETE FROM reactor_gateway.routes WHERE host = $1")
            .bind(host)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Load all custom domains.
    pub async fn load_custom_domains(&self) -> GatewayResult<Vec<CustomDomain>> {
        #[derive(FromRow)]
        struct DomainRow {
            host: String,
            project_id: uuid::Uuid,
            verification_token: String,
            verified_at: Option<DateTime<Utc>>,
            cert_status: String,
            created_at: DateTime<Utc>,
        }

        let rows: Vec<DomainRow> = sqlx::query_as(
            r#"
            SELECT host, project_id, verification_token, verified_at, cert_status, created_at
            FROM reactor_gateway.custom_domains
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let domains = rows
            .into_iter()
            .filter_map(|row| {
                let cert_status = CertStatus::from_str(&row.cert_status).ok()?;
                let project_id = ProjectId::from(row.project_id);

                Some(CustomDomain {
                    host: row.host,
                    project_id,
                    verification_token: row.verification_token,
                    verified_at: row.verified_at,
                    cert_status,
                    created_at: row.created_at,
                })
            })
            .collect();

        Ok(domains)
    }

    /// Get a custom domain by host.
    pub async fn get_custom_domain(&self, host: &str) -> GatewayResult<Option<CustomDomain>> {
        #[derive(FromRow)]
        struct DomainRow {
            host: String,
            project_id: uuid::Uuid,
            verification_token: String,
            verified_at: Option<DateTime<Utc>>,
            cert_status: String,
            created_at: DateTime<Utc>,
        }

        let row: Option<DomainRow> = sqlx::query_as(
            r#"
            SELECT host, project_id, verification_token, verified_at, cert_status, created_at
            FROM reactor_gateway.custom_domains
            WHERE host = $1
            "#,
        )
        .bind(host)
        .fetch_optional(&self.pool)
        .await?;

        let domain = row.and_then(|row| {
            let cert_status = CertStatus::from_str(&row.cert_status).ok()?;
            let project_id = ProjectId::from(row.project_id);

            Some(CustomDomain {
                host: row.host,
                project_id,
                verification_token: row.verification_token,
                verified_at: row.verified_at,
                cert_status,
                created_at: row.created_at,
            })
        });

        Ok(domain)
    }

    /// Insert a new custom domain.
    pub async fn insert_custom_domain(&self, domain: &CustomDomain) -> GatewayResult<()> {
        sqlx::query(
            r#"
            INSERT INTO reactor_gateway.custom_domains (host, project_id, verification_token, verified_at, cert_status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&domain.host)
        .bind(domain.project_id.as_uuid())
        .bind(&domain.verification_token)
        .bind(domain.verified_at)
        .bind(domain.cert_status.to_string())
        .bind(domain.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update a custom domain's verification status.
    pub async fn verify_custom_domain(&self, host: &str) -> GatewayResult<()> {
        sqlx::query(
            r#"
            UPDATE reactor_gateway.custom_domains
            SET verified_at = NOW()
            WHERE host = $1
            "#,
        )
        .bind(host)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update a custom domain's certificate status.
    pub async fn update_cert_status(&self, host: &str, status: CertStatus) -> GatewayResult<()> {
        sqlx::query(
            r#"
            UPDATE reactor_gateway.custom_domains
            SET cert_status = $2
            WHERE host = $1
            "#,
        )
        .bind(host)
        .bind(status.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a custom domain.
    pub async fn delete_custom_domain(&self, host: &str) -> GatewayResult<()> {
        sqlx::query("DELETE FROM reactor_gateway.custom_domains WHERE host = $1")
            .bind(host)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Check if a domain is verified (for on-demand TLS).
    pub async fn is_domain_verified(&self, host: &str) -> GatewayResult<bool> {
        let result: Option<(bool,)> = sqlx::query_as(
            r#"
            SELECT verified_at IS NOT NULL as verified
            FROM reactor_gateway.custom_domains
            WHERE host = $1
            "#,
        )
        .bind(host)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(v,)| v).unwrap_or(false))
    }

    /// Start listening for route changes.
    pub async fn listen(&self) -> GatewayResult<PgNotificationListener> {
        let mut listener = PgListener::connect_with(&self.pool).await?;
        listener.listen(&self.config.channel).await?;

        info!("Started listening on channel: {}", self.config.channel);

        Ok(PgNotificationListener { listener })
    }
}

#[async_trait]
impl RoutingTableLoader for PgRoutingStore {
    async fn load(&self) -> GatewayResult<RoutingTable> {
        let routes = self.load_routes().await?;
        Ok(RoutingTable::from_routes(routes))
    }
}

#[async_trait]
impl DomainVerifier for PgRoutingStore {
    async fn is_domain_allowed(&self, domain: &str) -> GatewayResult<bool> {
        self.is_domain_verified(domain).await
    }
}

/// Postgres notification listener for route changes.
pub struct PgNotificationListener {
    listener: PgListener,
}

impl PgNotificationListener {
    /// Wait for the next notification.
    pub async fn recv(&mut self) -> GatewayResult<String> {
        let notification = self.listener.recv().await?;
        Ok(notification.payload().to_string())
    }

    /// Try to receive without blocking.
    pub async fn try_recv(&mut self) -> GatewayResult<Option<String>> {
        match self.listener.try_recv().await {
            Ok(Some(notification)) => Ok(Some(notification.payload().to_string())),
            Ok(None) => Ok(None),
            Err(e) => Err(GatewayError::Database(e)),
        }
    }
}

/// Spawn a task that forwards notifications to a sync handle.
pub async fn spawn_notification_forwarder(
    store: Arc<PgRoutingStore>,
    sender: mpsc::Sender<crate::sync::SyncMessage>,
) -> GatewayResult<()> {
    let mut listener = store.listen().await?;

    tokio::spawn(async move {
        loop {
            match listener.recv().await {
                Ok(host) => {
                    debug!("Received route change notification: {}", host);
                    if sender
                        .send(crate::sync::SyncMessage::RouteChanged { host })
                        .await
                        .is_err()
                    {
                        warn!("Sync channel closed, stopping notification forwarder");
                        break;
                    }
                }
                Err(e) => {
                    error!("Error receiving notification: {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });

    Ok(())
}
