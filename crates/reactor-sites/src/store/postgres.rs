//! PostgreSQL implementation of SitesStore.

use super::*;
use sqlx::PgPool;

/// PostgreSQL-backed sites store.
pub struct PgSitesStore {
    pool: PgPool,
}

impl PgSitesStore {
    /// Create a new PostgreSQL sites store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl SitesStore for PgSitesStore {
    async fn create_site(&self, site: &NewSite) -> Result<Site, SitesError> {
        let id = uuid::Uuid::now_v7();
        let now = chrono::Utc::now();

        let row = sqlx::query_as::<_, Site>(
            r#"
            INSERT INTO _reactor_sites.sites (id, org_id, name, framework, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(site.org_id)
        .bind(&site.name)
        .bind(site.framework.to_string())
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    async fn get_site(&self, org_id: &Uuid, name: &str) -> Result<Option<Site>, SitesError> {
        let row = sqlx::query_as::<_, Site>(
            r#"
            SELECT * FROM _reactor_sites.sites
            WHERE org_id = $1 AND name = $2
            "#,
        )
        .bind(org_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn get_site_by_id(&self, id: &SiteId) -> Result<Option<Site>, SitesError> {
        let row = sqlx::query_as::<_, Site>(
            r#"
            SELECT * FROM _reactor_sites.sites
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn list_sites(&self, org_id: &Uuid) -> Result<Vec<Site>, SitesError> {
        let rows = sqlx::query_as::<_, Site>(
            r#"
            SELECT * FROM _reactor_sites.sites
            WHERE org_id = $1
            ORDER BY name
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn delete_site(&self, id: &SiteId) -> Result<(), SitesError> {
        sqlx::query("DELETE FROM _reactor_sites.sites WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn create_deployment(&self, d: &NewDeployment) -> Result<SiteDeployment, SitesError> {
        let id = uuid::Uuid::now_v7();
        let version = self.next_deployment_version(&d.site_id).await?;

        let row = sqlx::query_as::<_, SiteDeployment>(
            r#"
            INSERT INTO _reactor_sites.deployments 
                (id, site_id, version, manifest_json, status, deployed_by_user_id)
            VALUES ($1, $2, $3, $4, 'pending', $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(d.site_id)
        .bind(version)
        .bind(&d.manifest_json)
        .bind(d.deployed_by_user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    async fn get_deployment(
        &self,
        id: &SiteDeploymentId,
    ) -> Result<Option<SiteDeployment>, SitesError> {
        let row = sqlx::query_as::<_, SiteDeployment>(
            r#"
            SELECT * FROM _reactor_sites.deployments
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn current_deployment(
        &self,
        site_id: &SiteId,
    ) -> Result<Option<SiteDeployment>, SitesError> {
        let row = sqlx::query_as::<_, SiteDeployment>(
            r#"
            SELECT d.* FROM _reactor_sites.deployments d
            JOIN _reactor_sites.sites s ON s.current_deployment_id = d.id
            WHERE s.id = $1
            "#,
        )
        .bind(site_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn promote_deployment(&self, id: &SiteDeploymentId) -> Result<(), SitesError> {
        sqlx::query(
            r#"
            UPDATE _reactor_sites.sites
            SET current_deployment_id = $1, updated_at = now()
            WHERE id = (SELECT site_id FROM _reactor_sites.deployments WHERE id = $1)
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_deployments(
        &self,
        site_id: &SiteId,
        limit: u32,
    ) -> Result<Vec<SiteDeployment>, SitesError> {
        let rows = sqlx::query_as::<_, SiteDeployment>(
            r#"
            SELECT * FROM _reactor_sites.deployments
            WHERE site_id = $1
            ORDER BY deployed_at DESC
            LIMIT $2
            "#,
        )
        .bind(site_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn update_deployment_status(
        &self,
        id: &SiteDeploymentId,
        status: DeploymentStatus,
        detail: Option<&str>,
    ) -> Result<(), SitesError> {
        sqlx::query(
            r#"
            UPDATE _reactor_sites.deployments
            SET status = $2, status_detail = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status.to_string())
        .bind(detail)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_deployment_assets(
        &self,
        id: &SiteDeploymentId,
        count: i32,
        bytes: i64,
    ) -> Result<(), SitesError> {
        sqlx::query(
            r#"
            UPDATE _reactor_sites.deployments
            SET static_asset_count = $2, static_asset_bytes = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(count)
        .bind(bytes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn next_deployment_version(&self, site_id: &SiteId) -> Result<i64, SitesError> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COALESCE(MAX(version), 0) + 1
            FROM _reactor_sites.deployments
            WHERE site_id = $1
            "#,
        )
        .bind(site_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    async fn set_deployment_routes(
        &self,
        deployment_id: &SiteDeploymentId,
        routes: &[DeploymentRoute],
    ) -> Result<(), SitesError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM _reactor_sites.deployment_routes WHERE deployment_id = $1")
            .bind(deployment_id)
            .execute(&mut *tx)
            .await?;

        for route in routes {
            sqlx::query(
                r#"
                INSERT INTO _reactor_sites.deployment_routes 
                    (id, deployment_id, pattern, method_filter, route_kind, target_ref, cache_rules_json, priority)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(route.id)
            .bind(deployment_id)
            .bind(&route.pattern)
            .bind(&route.method_filter)
            .bind(&route.route_kind)
            .bind(&route.target_ref)
            .bind(&route.cache_rules_json)
            .bind(route.priority)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_deployment_routes(
        &self,
        deployment_id: &SiteDeploymentId,
    ) -> Result<Vec<DeploymentRoute>, SitesError> {
        let rows = sqlx::query_as::<_, DeploymentRoute>(
            r#"
            SELECT * FROM _reactor_sites.deployment_routes
            WHERE deployment_id = $1
            ORDER BY priority DESC
            "#,
        )
        .bind(deployment_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn add_deployment_function(
        &self,
        deployment_id: &SiteDeploymentId,
        function_id: &Uuid,
        role: &str,
    ) -> Result<(), SitesError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_sites.deployment_functions (deployment_id, function_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (deployment_id, function_id) DO NOTHING
            "#,
        )
        .bind(deployment_id)
        .bind(function_id)
        .bind(role)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_deployment_functions(
        &self,
        deployment_id: &SiteDeploymentId,
    ) -> Result<Vec<DeploymentFunction>, SitesError> {
        let rows = sqlx::query_as::<_, DeploymentFunction>(
            r#"
            SELECT * FROM _reactor_sites.deployment_functions
            WHERE deployment_id = $1
            "#,
        )
        .bind(deployment_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn create_domain(&self, d: &NewDomain) -> Result<Domain, SitesError> {
        let id = uuid::Uuid::now_v7();
        let token = uuid::Uuid::new_v4().to_string();

        let row = sqlx::query_as::<_, Domain>(
            r#"
            INSERT INTO _reactor_sites.domains 
                (id, site_id, host, verification_token, verification_method)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(d.site_id)
        .bind(&d.host)
        .bind(&token)
        .bind(&d.verification_method)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("domains_host_key") {
                    return SitesError::DomainTaken(d.host.clone());
                }
            }
            SitesError::Database(e)
        })?;

        Ok(row)
    }

    async fn get_domain(&self, host: &str) -> Result<Option<Domain>, SitesError> {
        let row = sqlx::query_as::<_, Domain>(
            r#"
            SELECT * FROM _reactor_sites.domains
            WHERE host = $1
            "#,
        )
        .bind(host)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn list_domains(&self, site_id: &SiteId) -> Result<Vec<Domain>, SitesError> {
        let rows = sqlx::query_as::<_, Domain>(
            r#"
            SELECT * FROM _reactor_sites.domains
            WHERE site_id = $1
            ORDER BY host
            "#,
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn update_domain_status(
        &self,
        id: &DomainId,
        status: DomainStatus,
        cert_ref: Option<&str>,
    ) -> Result<(), SitesError> {
        let verified_at = if status == DomainStatus::Verified || status == DomainStatus::Active {
            Some(chrono::Utc::now())
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE _reactor_sites.domains
            SET status = $2, tls_cert_ref = $3, verified_at = COALESCE($4, verified_at)
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status.to_string())
        .bind(cert_ref)
        .bind(verified_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_domain(&self, id: &DomainId) -> Result<(), SitesError> {
        sqlx::query("DELETE FROM _reactor_sites.domains WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_site_by_host(
        &self,
        host: &str,
    ) -> Result<Option<(Site, SiteDeployment)>, SitesError> {
        let row = sqlx::query_as::<_, SiteWithDeployment>(
            r#"
            SELECT 
                s.id as site_id,
                s.org_id,
                s.name as site_name,
                s.framework,
                s.current_deployment_id,
                s.created_at as site_created_at,
                s.updated_at as site_updated_at,
                d.id as deployment_id,
                d.site_id as deployment_site_id,
                d.version,
                d.manifest_json,
                d.status,
                d.status_detail,
                d.static_asset_count,
                d.static_asset_bytes,
                d.deployed_at,
                d.deployed_by_user_id
            FROM _reactor_sites.domains dom
            JOIN _reactor_sites.sites s ON s.id = dom.site_id
            JOIN _reactor_sites.deployments d ON d.id = s.current_deployment_id
            WHERE dom.host = $1 AND dom.status = 'active'
            "#,
        )
        .bind(host)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into_parts()))
    }

    async fn get_isr_entry(
        &self,
        site_id: &SiteId,
        path: &str,
    ) -> Result<Option<IsrCacheEntry>, SitesError> {
        let row = sqlx::query_as::<_, IsrCacheEntry>(
            r#"
            SELECT * FROM _reactor_sites.isr_cache
            WHERE site_id = $1 AND path = $2
            "#,
        )
        .bind(site_id)
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn set_isr_entry(&self, entry: &IsrCacheEntry) -> Result<(), SitesError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_sites.isr_cache 
                (site_id, path, deployment_id, body_storage_key, content_type, etag, tags, revalidate_after_secs, last_revalidated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (site_id, path) DO UPDATE
            SET deployment_id = $3, body_storage_key = $4, content_type = $5, etag = $6, 
                tags = $7, revalidate_after_secs = $8, last_revalidated_at = $9
            "#,
        )
        .bind(entry.site_id)
        .bind(&entry.path)
        .bind(entry.deployment_id)
        .bind(&entry.body_storage_key)
        .bind(&entry.content_type)
        .bind(&entry.etag)
        .bind(&entry.tags)
        .bind(entry.revalidate_after_secs)
        .bind(entry.last_revalidated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn invalidate_isr(&self, site_id: &SiteId, path_or_tag: &str) -> Result<u32, SitesError> {
        let result = sqlx::query(
            r#"
            DELETE FROM _reactor_sites.isr_cache
            WHERE site_id = $1 AND (path = $2 OR tags ? $2)
            "#,
        )
        .bind(site_id)
        .bind(path_or_tag)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as u32)
    }

    async fn get_site_policies(&self, site_id: &SiteId) -> Result<Vec<SitePolicy>, SitesError> {
        let rows = sqlx::query_as::<_, SitePolicy>(
            r#"
            SELECT * FROM _reactor_sites.policies
            WHERE site_id = $1
            ORDER BY name
            "#,
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn upsert_policy(&self, p: &NewSitePolicy) -> Result<SitePolicy, SitesError> {
        let id = uuid::Uuid::now_v7();

        let row = sqlx::query_as::<_, SitePolicy>(
            r#"
            INSERT INTO _reactor_sites.policies (id, site_id, name, using_expr_json, raw_text, sha256)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (site_id, name) DO UPDATE
            SET using_expr_json = $4, raw_text = $5, sha256 = $6, created_at = now()
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(p.site_id)
        .bind(&p.name)
        .bind(&p.using_expr_json)
        .bind(&p.raw_text)
        .bind(&p.sha256)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    async fn delete_policy(&self, id: &Uuid) -> Result<(), SitesError> {
        sqlx::query("DELETE FROM _reactor_sites.policies WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn write_audit_event(&self, event: &AuditEventCreate) -> Result<(), SitesError> {
        let id = uuid::Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO _reactor_sites.audit_events 
                (id, actor_user_id, actor_apikey_id, org_id, site_id, deployment_id, domain_id, event_type, details, request_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(id)
        .bind(event.actor_user_id)
        .bind(event.actor_apikey_id)
        .bind(event.org_id)
        .bind(event.site_id)
        .bind(event.deployment_id)
        .bind(event.domain_id)
        .bind(&event.event_type)
        .bind(&event.details)
        .bind(&event.request_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
