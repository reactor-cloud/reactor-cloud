//! PostgreSQL implementation of the FunctionsStore.

use super::*;
use sqlx::PgPool;

/// PostgreSQL-backed functions store.
#[derive(Clone)]
pub struct PgFunctionsStore {
    pool: PgPool,
}

impl PgFunctionsStore {
    /// Create a new PostgreSQL functions store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl FunctionsStore for PgFunctionsStore {
    async fn create_function(&self, input: FunctionCreate) -> Result<Function, FunctionsError> {
        let id = Uuid::now_v7();

        let function = sqlx::query_as::<_, Function>(
            r#"
            INSERT INTO _reactor_functions.functions (id, org_id, name, description, runtime)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(input.org_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.runtime)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                FunctionsError::FunctionExists(input.name.clone())
            }
            sqlx::Error::Database(db_err) if db_err.is_check_violation() => {
                FunctionsError::InvalidFunctionName(input.name.clone())
            }
            _ => FunctionsError::Database(e),
        })?;

        Ok(function)
    }

    async fn get_function(&self, id: FunctionId) -> Result<Option<Function>, FunctionsError> {
        let function = sqlx::query_as::<_, Function>(
            "SELECT * FROM _reactor_functions.functions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(function)
    }

    async fn get_function_by_name(
        &self,
        org_id: Uuid,
        name: &str,
    ) -> Result<Option<Function>, FunctionsError> {
        let function = sqlx::query_as::<_, Function>(
            "SELECT * FROM _reactor_functions.functions WHERE org_id = $1 AND name = $2",
        )
        .bind(org_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(function)
    }

    async fn list_functions(&self, org_id: Uuid) -> Result<Vec<Function>, FunctionsError> {
        let functions = sqlx::query_as::<_, Function>(
            "SELECT * FROM _reactor_functions.functions WHERE org_id = $1 ORDER BY name",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(functions)
    }

    async fn delete_function(&self, id: FunctionId) -> Result<bool, FunctionsError> {
        let result = sqlx::query("DELETE FROM _reactor_functions.functions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_current_deployment(
        &self,
        function_id: FunctionId,
        deployment_id: Option<DeploymentId>,
    ) -> Result<(), FunctionsError> {
        sqlx::query(
            r#"
            UPDATE _reactor_functions.functions
            SET current_deployment_id = $1, updated_at = now()
            WHERE id = $2
            "#,
        )
        .bind(deployment_id)
        .bind(function_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn create_deployment(
        &self,
        input: DeploymentCreate,
    ) -> Result<Deployment, FunctionsError> {
        let id = Uuid::now_v7();
        let version = self.next_deployment_version(input.function_id).await?;

        let deployment = sqlx::query_as::<_, Deployment>(
            r#"
            INSERT INTO _reactor_functions.deployments 
            (id, function_id, version, bundle_bucket, bundle_object_key, bundle_sha256, bundle_size, manifest_json, status, deployed_by_user_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', $9)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(input.function_id)
        .bind(version)
        .bind(&input.bundle_bucket)
        .bind(&input.bundle_object_key)
        .bind(&input.bundle_sha256)
        .bind(input.bundle_size)
        .bind(&input.manifest_json)
        .bind(input.deployed_by_user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(deployment)
    }

    async fn get_deployment(&self, id: DeploymentId) -> Result<Option<Deployment>, FunctionsError> {
        let deployment = sqlx::query_as::<_, Deployment>(
            "SELECT * FROM _reactor_functions.deployments WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(deployment)
    }

    async fn get_current_deployment(
        &self,
        function_id: FunctionId,
    ) -> Result<Option<Deployment>, FunctionsError> {
        let deployment = sqlx::query_as::<_, Deployment>(
            r#"
            SELECT d.* FROM _reactor_functions.deployments d
            JOIN _reactor_functions.functions f ON f.current_deployment_id = d.id
            WHERE f.id = $1
            "#,
        )
        .bind(function_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(deployment)
    }

    async fn list_deployments(
        &self,
        function_id: FunctionId,
    ) -> Result<Vec<Deployment>, FunctionsError> {
        let deployments = sqlx::query_as::<_, Deployment>(
            "SELECT * FROM _reactor_functions.deployments WHERE function_id = $1 ORDER BY deployed_at DESC",
        )
        .bind(function_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(deployments)
    }

    async fn update_deployment_status(
        &self,
        id: DeploymentId,
        status: DeploymentStatus,
        status_detail: Option<String>,
        runtime_ref: Option<String>,
    ) -> Result<(), FunctionsError> {
        sqlx::query(
            r#"
            UPDATE _reactor_functions.deployments
            SET status = $1, status_detail = $2, runtime_ref = $3
            WHERE id = $4
            "#,
        )
        .bind(status.to_string())
        .bind(status_detail)
        .bind(runtime_ref)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn next_deployment_version(
        &self,
        function_id: FunctionId,
    ) -> Result<i64, FunctionsError> {
        let row: (Option<i64>,) = sqlx::query_as(
            "SELECT MAX(version) FROM _reactor_functions.deployments WHERE function_id = $1",
        )
        .bind(function_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0.unwrap_or(0) + 1)
    }

    async fn get_env(&self, function_id: FunctionId) -> Result<Vec<EnvVar>, FunctionsError> {
        let env_vars = sqlx::query_as::<_, EnvVar>(
            "SELECT * FROM _reactor_functions.env WHERE function_id = $1 ORDER BY key",
        )
        .bind(function_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(env_vars)
    }

    async fn get_env_var(
        &self,
        function_id: FunctionId,
        key: &str,
    ) -> Result<Option<EnvVar>, FunctionsError> {
        let env_var = sqlx::query_as::<_, EnvVar>(
            "SELECT * FROM _reactor_functions.env WHERE function_id = $1 AND key = $2",
        )
        .bind(function_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(env_var)
    }

    async fn upsert_env(
        &self,
        function_id: FunctionId,
        key: &str,
        value_plaintext: Option<String>,
        value_encrypted: Option<Vec<u8>>,
        is_secret: bool,
    ) -> Result<(), FunctionsError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_functions.env (function_id, key, value_plaintext, value_encrypted, is_secret, last_updated_at)
            VALUES ($1, $2, $3, $4, $5, now())
            ON CONFLICT (function_id, key) DO UPDATE
            SET value_plaintext = EXCLUDED.value_plaintext,
                value_encrypted = EXCLUDED.value_encrypted,
                is_secret = EXCLUDED.is_secret,
                last_updated_at = now()
            "#,
        )
        .bind(function_id)
        .bind(key)
        .bind(value_plaintext)
        .bind(value_encrypted)
        .bind(is_secret)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_check_violation() => {
                FunctionsError::EnvKeyInvalid(key.to_string())
            }
            _ => FunctionsError::Database(e),
        })?;

        Ok(())
    }

    async fn delete_env(&self, function_id: FunctionId, key: &str) -> Result<bool, FunctionsError> {
        let result = sqlx::query(
            "DELETE FROM _reactor_functions.env WHERE function_id = $1 AND key = $2",
        )
        .bind(function_id)
        .bind(key)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn create_policy(
        &self,
        function_id: FunctionId,
        name: &str,
        using_expr_json: Option<serde_json::Value>,
        raw_text: &str,
        sha256: Vec<u8>,
    ) -> Result<Policy, FunctionsError> {
        let id = Uuid::now_v7();

        let policy = sqlx::query_as::<_, Policy>(
            r#"
            INSERT INTO _reactor_functions.policies (id, function_id, name, using_expr_json, raw_text, sha256)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(function_id)
        .bind(name)
        .bind(using_expr_json)
        .bind(raw_text)
        .bind(&sha256)
        .fetch_one(&self.pool)
        .await?;

        Ok(policy)
    }

    async fn get_policies(&self, function_id: FunctionId) -> Result<Vec<Policy>, FunctionsError> {
        let policies = sqlx::query_as::<_, Policy>(
            "SELECT * FROM _reactor_functions.policies WHERE function_id = $1 ORDER BY name",
        )
        .bind(function_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(policies)
    }

    async fn delete_policy(
        &self,
        function_id: FunctionId,
        name: &str,
    ) -> Result<bool, FunctionsError> {
        let result = sqlx::query(
            "DELETE FROM _reactor_functions.policies WHERE function_id = $1 AND name = $2",
        )
        .bind(function_id)
        .bind(name)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn record_invocation(&self, input: InvocationCreate) -> Result<(), FunctionsError> {
        let id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO _reactor_functions.invocations 
            (id, deployment_id, function_id, org_id, actor_user_id, actor_apikey_id, request_id, method, sub_path, status_code, duration_ms, cold_start, bytes_in, bytes_out, error_code)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(id)
        .bind(input.deployment_id)
        .bind(input.function_id)
        .bind(input.org_id)
        .bind(input.actor_user_id)
        .bind(input.actor_apikey_id)
        .bind(&input.request_id)
        .bind(&input.method)
        .bind(&input.sub_path)
        .bind(input.status_code)
        .bind(input.duration_ms)
        .bind(input.cold_start)
        .bind(input.bytes_in)
        .bind(input.bytes_out)
        .bind(&input.error_code)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn record_audit_event(&self, input: AuditEventCreate) -> Result<(), FunctionsError> {
        let id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO _reactor_functions.audit_events 
            (id, actor_user_id, actor_apikey_id, org_id, function_id, deployment_id, event_type, details, request_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(input.actor_user_id)
        .bind(input.actor_apikey_id)
        .bind(input.org_id)
        .bind(input.function_id)
        .bind(input.deployment_id)
        .bind(&input.event_type)
        .bind(&input.details)
        .bind(&input.request_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl FunctionsTx for PgFunctionsStore {
    async fn transaction<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'c> FnOnce(
                &'c mut sqlx::Transaction<'static, sqlx::Postgres>,
            ) -> futures::future::BoxFuture<'c, Result<T, E>>
            + Send,
        T: Send,
        E: From<sqlx::Error> + Send,
    {
        let mut tx = self.pool.begin().await?;
        let result = f(&mut tx).await?;
        tx.commit().await?;
        Ok(result)
    }
}
