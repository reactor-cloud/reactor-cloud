//! PostgreSQL implementation of ConnectStore.

use super::*;
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// PostgreSQL Connect store.
#[derive(Clone)]
pub struct PgConnectStore {
    pool: PgPool,
}

impl PgConnectStore {
    /// Create a new Postgres store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Run migrations.
    pub async fn migrate(&self) -> Result<(), ConnectError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| ConnectError::Database(e.into()))?;
        Ok(())
    }
}

/// PostgreSQL transaction.
pub struct PgConnectTx<'a> {
    tx: Transaction<'a, Postgres>,
}

#[async_trait]
impl ConnectTx for PgConnectTx<'_> {
    async fn execute_raw(&mut self, sql: &str, _params: &[&str]) -> Result<u64, ConnectError> {
        let result = sqlx::query(sql).execute(&mut *self.tx).await?;
        Ok(result.rows_affected())
    }

    async fn commit(self) -> Result<(), ConnectError> {
        self.tx.commit().await?;
        Ok(())
    }

    async fn rollback(self) -> Result<(), ConnectError> {
        self.tx.rollback().await?;
        Ok(())
    }
}

#[async_trait]
impl ConnectStore for PgConnectStore {
    type Tx<'a> = PgConnectTx<'a>;

    async fn begin(&self) -> Result<Self::Tx<'_>, ConnectError> {
        let tx = self.pool.begin().await?;
        Ok(PgConnectTx { tx })
    }

    async fn create_instance(
        &self,
        org_id: &OrgId,
        instance: &NewInstance,
    ) -> Result<Instance, ConnectError> {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO _reactor_connect.instances 
                (id, org_id, type_id, name, config_json, credential_state, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, 'pending', $6, $6)
            "#,
        )
        .bind(id)
        .bind(org_id)
        .bind(&instance.type_id)
        .bind(&instance.name)
        .bind(&instance.config_json)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(Instance {
            id,
            org_id: *org_id,
            type_id: instance.type_id.clone(),
            name: instance.name.clone(),
            config_json: instance.config_json.clone(),
            vault_ref: None,
            credential_state: "pending".to_string(),
            credential_error: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_instance(
        &self,
        org_id: &OrgId,
        name: &str,
    ) -> Result<Option<Instance>, ConnectError> {
        let row = sqlx::query_as::<_, InstanceRow>(
            r#"
            SELECT id, org_id, type_id, name, config_json, vault_ref, 
                   credential_state, credential_error, enabled, created_at, updated_at
            FROM _reactor_connect.instances
            WHERE org_id = $1 AND name = $2
            "#,
        )
        .bind(org_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn get_instance_by_id(&self, id: &InstanceId) -> Result<Option<Instance>, ConnectError> {
        let row = sqlx::query_as::<_, InstanceRow>(
            r#"
            SELECT id, org_id, type_id, name, config_json, vault_ref,
                   credential_state, credential_error, enabled, created_at, updated_at
            FROM _reactor_connect.instances
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn list_instances(&self, org_id: &OrgId) -> Result<Vec<Instance>, ConnectError> {
        let rows = sqlx::query_as::<_, InstanceRow>(
            r#"
            SELECT id, org_id, type_id, name, config_json, vault_ref,
                   credential_state, credential_error, enabled, created_at, updated_at
            FROM _reactor_connect.instances
            WHERE org_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update_instance_credentials(
        &self,
        id: &InstanceId,
        vault_ref: &str,
        state: &str,
        error: Option<&str>,
    ) -> Result<(), ConnectError> {
        sqlx::query(
            r#"
            UPDATE _reactor_connect.instances
            SET vault_ref = $2, credential_state = $3, credential_error = $4, updated_at = now()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(vault_ref)
        .bind(state)
        .bind(error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_instance(&self, id: &InstanceId) -> Result<(), ConnectError> {
        sqlx::query("DELETE FROM _reactor_connect.instances WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn create_connection(
        &self,
        org_id: &OrgId,
        conn: &NewConnection,
    ) -> Result<Connection, ConnectError> {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO _reactor_connect.connections
                (id, org_id, name, source_instance_id, source_kind, source_config_json,
                 dest_instance_id, dest_kind, dest_config_json,
                 schedule_kind, schedule_config_json, options_json, direction,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)
            "#,
        )
        .bind(id)
        .bind(org_id)
        .bind(&conn.name)
        .bind(conn.source_instance_id)
        .bind(&conn.source_kind)
        .bind(&conn.source_config_json)
        .bind(conn.dest_instance_id)
        .bind(&conn.dest_kind)
        .bind(&conn.dest_config_json)
        .bind(&conn.schedule_kind)
        .bind(&conn.schedule_config_json)
        .bind(&conn.options_json)
        .bind(&conn.direction)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(Connection {
            id,
            org_id: *org_id,
            name: conn.name.clone(),
            source_instance_id: conn.source_instance_id,
            source_kind: conn.source_kind.clone(),
            source_config_json: conn.source_config_json.clone(),
            dest_instance_id: conn.dest_instance_id,
            dest_kind: conn.dest_kind.clone(),
            dest_config_json: conn.dest_config_json.clone(),
            schedule_kind: conn.schedule_kind.clone(),
            schedule_config_json: conn.schedule_config_json.clone(),
            options_json: conn.options_json.clone(),
            enabled: false,
            direction: conn.direction.clone(),
            last_sync_at: None,
            job_name: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_connection(
        &self,
        org_id: &OrgId,
        name: &str,
    ) -> Result<Option<Connection>, ConnectError> {
        let row = sqlx::query_as::<_, ConnectionRow>(
            r#"
            SELECT id, org_id, name, source_instance_id, source_kind, source_config_json,
                   dest_instance_id, dest_kind, dest_config_json, schedule_kind, schedule_config_json,
                   options_json, enabled, direction, last_sync_at, job_name, created_at, updated_at
            FROM _reactor_connect.connections
            WHERE org_id = $1 AND name = $2
            "#,
        )
        .bind(org_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn get_connection_by_id(&self, id: &ConnectionId) -> Result<Option<Connection>, ConnectError> {
        let row = sqlx::query_as::<_, ConnectionRow>(
            r#"
            SELECT id, org_id, name, source_instance_id, source_kind, source_config_json,
                   dest_instance_id, dest_kind, dest_config_json, schedule_kind, schedule_config_json,
                   options_json, enabled, direction, last_sync_at, job_name, created_at, updated_at
            FROM _reactor_connect.connections
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn list_connections(&self, org_id: &OrgId) -> Result<Vec<Connection>, ConnectError> {
        let rows = sqlx::query_as::<_, ConnectionRow>(
            r#"
            SELECT id, org_id, name, source_instance_id, source_kind, source_config_json,
                   dest_instance_id, dest_kind, dest_config_json, schedule_kind, schedule_config_json,
                   options_json, enabled, direction, last_sync_at, job_name, created_at, updated_at
            FROM _reactor_connect.connections
            WHERE org_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn set_connection_enabled(
        &self,
        id: &ConnectionId,
        enabled: bool,
    ) -> Result<(), ConnectError> {
        sqlx::query(
            "UPDATE _reactor_connect.connections SET enabled = $2, updated_at = now() WHERE id = $1",
        )
        .bind(id)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_connection(&self, id: &ConnectionId) -> Result<(), ConnectError> {
        sqlx::query("DELETE FROM _reactor_connect.connections WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn create_receiver(
        &self,
        org_id: &OrgId,
        receiver: &NewReceiver,
    ) -> Result<Receiver, ConnectError> {
        let id = Uuid::now_v7();
        let token = generate_receiver_token();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO _reactor_connect.receivers
                (id, instance_id, org_id, webhook_name, token, dispatch_kind, dispatch_config_json, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(receiver.instance_id)
        .bind(org_id)
        .bind(&receiver.webhook_name)
        .bind(&token)
        .bind(&receiver.dispatch_kind)
        .bind(&receiver.dispatch_config_json)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(Receiver {
            id,
            instance_id: receiver.instance_id,
            org_id: *org_id,
            webhook_name: receiver.webhook_name.clone(),
            token,
            dispatch_kind: receiver.dispatch_kind.clone(),
            dispatch_config_json: receiver.dispatch_config_json.clone(),
            enabled: true,
            last_received_at: None,
            created_at: now,
        })
    }

    async fn get_receiver_by_token(&self, token: &str) -> Result<Option<Receiver>, ConnectError> {
        let row = sqlx::query_as::<_, ReceiverRow>(
            r#"
            SELECT id, instance_id, org_id, webhook_name, token, dispatch_kind,
                   dispatch_config_json, enabled, last_received_at, created_at
            FROM _reactor_connect.receivers
            WHERE token = $1
            "#,
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn list_receivers(&self, instance_id: &InstanceId) -> Result<Vec<Receiver>, ConnectError> {
        let rows = sqlx::query_as::<_, ReceiverRow>(
            r#"
            SELECT id, instance_id, org_id, webhook_name, token, dispatch_kind,
                   dispatch_config_json, enabled, last_received_at, created_at
            FROM _reactor_connect.receivers
            WHERE instance_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn delete_receiver(&self, id: &ReceiverId) -> Result<(), ConnectError> {
        sqlx::query("DELETE FROM _reactor_connect.receivers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_state(
        &self,
        connection_id: &ConnectionId,
        stream_name: &str,
    ) -> Result<Option<StateBundle>, ConnectError> {
        let row = sqlx::query_as::<_, StateRow>(
            r#"
            SELECT connection_id, stream_name, state_json, updated_at
            FROM _reactor_connect.connection_state
            WHERE connection_id = $1 AND stream_name = $2
            "#,
        )
        .bind(connection_id)
        .bind(stream_name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn put_state(
        &self,
        connection_id: &ConnectionId,
        stream_name: &str,
        state: &serde_json::Value,
    ) -> Result<(), ConnectError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_connect.connection_state (connection_id, stream_name, state_json, updated_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (connection_id, stream_name) DO UPDATE
            SET state_json = $3, updated_at = now()
            "#,
        )
        .bind(connection_id)
        .bind(stream_name)
        .bind(state)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn record_run(&self, run: &SyncRunRecord) -> Result<(), ConnectError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_connect.sync_runs
                (id, connection_id, org_id, jobs_run_id, status, records_read, records_written,
                 error_code, error_message, error_suggested_fix, started_at, finished_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(run.id)
        .bind(run.connection_id)
        .bind(run.org_id)
        .bind(run.jobs_run_id)
        .bind(&run.status)
        .bind(&run.records_read)
        .bind(&run.records_written)
        .bind(&run.error_code)
        .bind(&run.error_message)
        .bind(&run.error_suggested_fix)
        .bind(run.started_at)
        .bind(run.finished_at)
        .bind(run.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_runs(
        &self,
        connection_id: &ConnectionId,
        limit: u32,
    ) -> Result<Vec<SyncRunRecord>, ConnectError> {
        let rows = sqlx::query_as::<_, SyncRunRow>(
            r#"
            SELECT id, connection_id, org_id, jobs_run_id, status, records_read, records_written,
                   error_code, error_message, error_suggested_fix, started_at, finished_at, created_at
            FROM _reactor_connect.sync_runs
            WHERE connection_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(connection_id)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn record_invocation(&self, inv: &ActionInvocationRecord) -> Result<(), ConnectError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_connect.action_invocations
                (id, instance_id, org_id, action_name, input_hash, idempotency_key,
                 dry_run, status, duration_ms, error_code, error_message, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(inv.id)
        .bind(inv.instance_id)
        .bind(inv.org_id)
        .bind(&inv.action_name)
        .bind(&inv.input_hash)
        .bind(&inv.idempotency_key)
        .bind(inv.dry_run)
        .bind(&inv.status)
        .bind(inv.duration_ms)
        .bind(&inv.error_code)
        .bind(&inv.error_message)
        .bind(inv.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), ConnectError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_connect.audit_events
                (id, ts, actor_user_id, actor_apikey_id, org_id, instance_id,
                 connection_id, receiver_id, event_type, details, request_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(event.id)
        .bind(event.ts)
        .bind(event.actor_user_id)
        .bind(event.actor_apikey_id)
        .bind(event.org_id)
        .bind(event.instance_id)
        .bind(event.connection_id)
        .bind(event.receiver_id)
        .bind(&event.event_type)
        .bind(&event.details)
        .bind(&event.request_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// Generate a secure receiver token.
fn generate_receiver_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

// Row types for sqlx
#[derive(sqlx::FromRow)]
struct InstanceRow {
    id: Uuid,
    org_id: Uuid,
    type_id: String,
    name: String,
    config_json: serde_json::Value,
    vault_ref: Option<String>,
    credential_state: String,
    credential_error: Option<String>,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<InstanceRow> for Instance {
    fn from(row: InstanceRow) -> Self {
        Self {
            id: row.id,
            org_id: row.org_id,
            type_id: row.type_id,
            name: row.name,
            config_json: row.config_json,
            vault_ref: row.vault_ref,
            credential_state: row.credential_state,
            credential_error: row.credential_error,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ConnectionRow {
    id: Uuid,
    org_id: Uuid,
    name: String,
    source_instance_id: Option<Uuid>,
    source_kind: String,
    source_config_json: serde_json::Value,
    dest_instance_id: Option<Uuid>,
    dest_kind: String,
    dest_config_json: serde_json::Value,
    schedule_kind: String,
    schedule_config_json: serde_json::Value,
    options_json: serde_json::Value,
    enabled: bool,
    direction: String,
    last_sync_at: Option<DateTime<Utc>>,
    job_name: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ConnectionRow> for Connection {
    fn from(row: ConnectionRow) -> Self {
        Self {
            id: row.id,
            org_id: row.org_id,
            name: row.name,
            source_instance_id: row.source_instance_id,
            source_kind: row.source_kind,
            source_config_json: row.source_config_json,
            dest_instance_id: row.dest_instance_id,
            dest_kind: row.dest_kind,
            dest_config_json: row.dest_config_json,
            schedule_kind: row.schedule_kind,
            schedule_config_json: row.schedule_config_json,
            options_json: row.options_json,
            enabled: row.enabled,
            direction: row.direction,
            last_sync_at: row.last_sync_at,
            job_name: row.job_name,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ReceiverRow {
    id: Uuid,
    instance_id: Uuid,
    org_id: Uuid,
    webhook_name: String,
    token: String,
    dispatch_kind: String,
    dispatch_config_json: serde_json::Value,
    enabled: bool,
    last_received_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<ReceiverRow> for Receiver {
    fn from(row: ReceiverRow) -> Self {
        Self {
            id: row.id,
            instance_id: row.instance_id,
            org_id: row.org_id,
            webhook_name: row.webhook_name,
            token: row.token,
            dispatch_kind: row.dispatch_kind,
            dispatch_config_json: row.dispatch_config_json,
            enabled: row.enabled,
            last_received_at: row.last_received_at,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct StateRow {
    connection_id: Uuid,
    stream_name: String,
    state_json: serde_json::Value,
    updated_at: DateTime<Utc>,
}

impl From<StateRow> for StateBundle {
    fn from(row: StateRow) -> Self {
        Self {
            connection_id: row.connection_id,
            stream_name: row.stream_name,
            state_json: row.state_json,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SyncRunRow {
    id: Uuid,
    connection_id: Uuid,
    org_id: Uuid,
    jobs_run_id: Option<Uuid>,
    status: String,
    records_read: serde_json::Value,
    records_written: serde_json::Value,
    error_code: Option<String>,
    error_message: Option<String>,
    error_suggested_fix: Option<String>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<SyncRunRow> for SyncRunRecord {
    fn from(row: SyncRunRow) -> Self {
        Self {
            id: row.id,
            connection_id: row.connection_id,
            org_id: row.org_id,
            jobs_run_id: row.jobs_run_id,
            status: row.status,
            records_read: row.records_read,
            records_written: row.records_written,
            error_code: row.error_code,
            error_message: row.error_message,
            error_suggested_fix: row.error_suggested_fix,
            started_at: row.started_at,
            finished_at: row.finished_at,
            created_at: row.created_at,
        }
    }
}
