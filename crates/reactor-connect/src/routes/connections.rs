//! Connection management endpoints (v0.2+, stubs for now).

use crate::error::ConnectError;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::{ConnectStore, Connection, NewConnection};
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

/// Create connection request.
#[derive(Debug, Deserialize)]
pub struct CreateConnectionRequest {
    /// Connection name.
    pub name: String,
    /// Source configuration.
    pub source: SourceConfig,
    /// Destination configuration.
    pub destination: DestinationConfig,
    /// Schedule configuration.
    #[serde(default)]
    pub schedule: ScheduleConfig,
    /// Options.
    #[serde(default)]
    pub options: ConnectionOptions,
}

/// Source configuration.
#[derive(Debug, Deserialize)]
pub struct SourceConfig {
    /// Instance name (if source is an instance).
    pub instance: Option<String>,
    /// Source kind.
    #[serde(default = "default_instance_kind")]
    pub kind: String,
    /// Streams to sync.
    #[serde(default)]
    pub streams: Vec<StreamConfig>,
}

/// Stream configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Stream name.
    pub name: String,
    /// Sync mode.
    pub mode: String,
    /// Primary key paths.
    #[serde(default)]
    pub primary_key: Option<Vec<Vec<String>>>,
}

/// Destination configuration.
#[derive(Debug, Deserialize)]
pub struct DestinationConfig {
    /// Instance name (if destination is an instance).
    pub instance: Option<String>,
    /// Destination kind.
    #[serde(default = "default_data_kind")]
    pub kind: String,
    /// Table name (if kind is data).
    pub table: Option<String>,
}

/// Schedule configuration.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ScheduleConfig {
    /// Cron expression.
    pub cron: Option<String>,
    /// Event trigger.
    pub on_event: Option<String>,
    /// Manual trigger only.
    #[serde(default)]
    pub manual: bool,
}

/// Connection options.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConnectionOptions {
    /// Schema drift handling.
    #[serde(default = "default_schema_drift")]
    pub schema_drift: String,
    /// Maximum rows per run.
    pub max_rows_per_run: Option<u64>,
}

fn default_instance_kind() -> String {
    "instance".to_string()
}

fn default_data_kind() -> String {
    "data".to_string()
}

fn default_schema_drift() -> String {
    "block_until_approved".to_string()
}

/// POST /connect/v1/connections
pub async fn create<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Json(req): Json<CreateConnectionRequest>,
) -> Result<Json<Connection>, ConnectError> {
    // Validate name
    if !crate::CONNECTION_NAME_REGEX.is_match(&req.name) {
        return Err(ConnectError::InvalidInput(format!(
            "Invalid connection name: {}",
            req.name
        )));
    }

    // Resolve source instance ID
    let source_instance_id = if let Some(name) = &req.source.instance {
        let inst = state
            .store
            .get_instance(ctx.active_org(), name)
            .await?
            .ok_or_else(|| ConnectError::InstanceNotFound(name.clone()))?;
        Some(inst.id)
    } else {
        None
    };

    // Resolve destination instance ID
    let dest_instance_id = if let Some(name) = &req.destination.instance {
        let inst = state
            .store
            .get_instance(ctx.active_org(), name)
            .await?
            .ok_or_else(|| ConnectError::InstanceNotFound(name.clone()))?;
        Some(inst.id)
    } else {
        None
    };

    // Determine schedule kind
    let schedule_kind = if req.schedule.cron.is_some() {
        "cron"
    } else if req.schedule.on_event.is_some() {
        "on_event"
    } else {
        "manual"
    };

    let connection = state
        .store
        .create_connection(
            ctx.active_org(),
            &NewConnection {
                name: req.name,
                source_instance_id,
                source_kind: req.source.kind,
                source_config_json: serde_json::to_value(&req.source.streams)?,
                dest_instance_id,
                dest_kind: req.destination.kind,
                dest_config_json: serde_json::json!({
                    "table": req.destination.table,
                }),
                schedule_kind: schedule_kind.to_string(),
                schedule_config_json: serde_json::to_value(&req.schedule)?,
                options_json: serde_json::to_value(&req.options)?,
                direction: "inbound".to_string(),
            },
        )
        .await?;

    // TODO: Create reactor-jobs job for scheduled connections

    Ok(Json(connection))
}

/// GET /connect/v1/connections
pub async fn list<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
) -> Result<Json<Vec<Connection>>, ConnectError> {
    let connections = state.store.list_connections(ctx.active_org()).await?;
    Ok(Json(connections))
}

/// GET /connect/v1/connections/:name
pub async fn show<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<Json<Connection>, ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name))?;
    Ok(Json(connection))
}

/// Update connection request.
#[derive(Debug, Deserialize)]
pub struct UpdateConnectionRequest {
    /// Enable/disable.
    pub enabled: Option<bool>,
    /// Schedule update.
    pub schedule: Option<ScheduleConfig>,
    /// Options update.
    pub options: Option<ConnectionOptions>,
}

/// PATCH /connect/v1/connections/:name
pub async fn update<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
    Json(req): Json<UpdateConnectionRequest>,
) -> Result<Json<Connection>, ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name.clone()))?;

    if let Some(enabled) = req.enabled {
        state
            .store
            .set_connection_enabled(&connection.id, enabled)
            .await?;
    }

    // TODO: Update schedule and options

    // Reload
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name))?;

    Ok(Json(connection))
}

/// DELETE /connect/v1/connections/:name
pub async fn delete<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<(), ConnectError> {
    let connection = state
        .store
        .get_connection(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::ConnectionNotFound(name))?;

    // TODO: Delete reactor-jobs job

    state.store.delete_connection(&connection.id).await?;
    Ok(())
}
