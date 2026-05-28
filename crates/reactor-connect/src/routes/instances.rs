//! Instance management endpoints.

use crate::error::ConnectError;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::{ConnectStore, Instance, NewInstance};
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

/// Create instance request.
#[derive(Debug, Deserialize)]
pub struct CreateInstanceRequest {
    /// Connector type ID.
    pub type_id: String,
    /// Instance name.
    pub name: String,
    /// Configuration (non-secret).
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Create instance response.
#[derive(Debug, Serialize)]
pub struct CreateInstanceResponse {
    /// Created instance.
    pub instance: Instance,
    /// OAuth URL if OAuth2 auth is required.
    pub oauth_url: Option<String>,
}

/// POST /connect/v1/instances
pub async fn create<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Json(req): Json<CreateInstanceRequest>,
) -> Result<Json<CreateInstanceResponse>, ConnectError> {
    // Validate name
    if !crate::INSTANCE_NAME_REGEX.is_match(&req.name) {
        return Err(ConnectError::InvalidInstanceName(req.name));
    }

    // Check connector type exists
    let descriptor = state.runtime.descriptor(&req.type_id).await?;

    // Check for duplicate
    if state
        .store
        .get_instance(ctx.active_org(), &req.name)
        .await?
        .is_some()
    {
        return Err(ConnectError::InstanceAlreadyExists(req.name));
    }

    // Create instance
    let instance = state
        .store
        .create_instance(
            ctx.active_org(),
            &NewInstance {
                type_id: req.type_id,
                name: req.name,
                config_json: req.config,
            },
        )
        .await?;

    // Generate OAuth URL if needed
    let oauth_url = if descriptor.auth.requires_oauth() {
        // TODO: Generate OAuth URL with PKCE
        None
    } else {
        None
    };

    Ok(Json(CreateInstanceResponse { instance, oauth_url }))
}

/// GET /connect/v1/instances
pub async fn list<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
) -> Result<Json<Vec<Instance>>, ConnectError> {
    let instances = state.store.list_instances(ctx.active_org()).await?;
    Ok(Json(instances))
}

/// Instance with descriptor.
#[derive(Debug, Serialize)]
pub struct InstanceWithDescriptor {
    /// Instance details.
    #[serde(flatten)]
    pub instance: Instance,
    /// Connector descriptor.
    pub descriptor: crate::descriptor::ConnectorDescriptor,
}

/// GET /connect/v1/instances/:name
pub async fn show<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<Json<InstanceWithDescriptor>, ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    let descriptor = state.runtime.descriptor(&instance.type_id).await?;

    Ok(Json(InstanceWithDescriptor {
        instance,
        descriptor,
    }))
}

/// DELETE /connect/v1/instances/:name
pub async fn delete<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<(), ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    // TODO: Delete vault credentials
    // TODO: Disable connections referencing this instance

    state.store.delete_instance(&instance.id).await?;
    Ok(())
}

/// Credentials request.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CredentialsRequest {
    /// OAuth2 callback with code.
    OAuthCallback {
        code: String,
        redirect_uri: String,
    },
    /// Personal access token.
    Pat {
        personal_access_token: String,
    },
    /// Client credentials (OAuth2 app).
    ClientCredentials {
        client_id: String,
        client_secret: String,
        #[serde(default)]
        refresh_token: Option<String>,
    },
}

/// Credentials response.
#[derive(Debug, Serialize)]
pub struct CredentialsResponse {
    /// Updated instance.
    pub instance: Instance,
    /// Credential state.
    pub credential_state: String,
}

/// POST /connect/v1/instances/:name/credentials
pub async fn credentials<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
    Json(req): Json<CredentialsRequest>,
) -> Result<Json<CredentialsResponse>, ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name.clone()))?;

    // Build credentials based on request type
    let credentials = match req {
        CredentialsRequest::OAuthCallback { code, redirect_uri } => {
            // TODO: Exchange code for tokens
            crate::credentials::Credentials::OAuth2 {
                access_token: code, // Placeholder
                refresh_token: None,
                expires_at: None,
                client_id: String::new(),
                client_secret: String::new(),
            }
        }
        CredentialsRequest::Pat { personal_access_token } => {
            crate::credentials::Credentials::Pat {
                token: personal_access_token,
            }
        }
        CredentialsRequest::ClientCredentials {
            client_id,
            client_secret,
            refresh_token,
        } => crate::credentials::Credentials::OAuth2 {
            access_token: String::new(),
            refresh_token,
            expires_at: None,
            client_id,
            client_secret,
        },
    };

    // Store credentials in vault
    let vault_ref = format!(
        "connect/{}/instances/{}",
        ctx.active_org(),
        instance.id
    );
    
    // Use a nil project ID for now - in production this comes from tenant context
    let project_id = reactor_core::ProjectId::nil();
    
    let secret_data = serde_json::to_vec(&credentials)?;
    state
        .vault
        .put_secret(
            &project_id,
            &vault_ref,
            reactor_core::SecretValue::new(secret_data),
        )
        .await
        .map_err(|e| ConnectError::Vault(e.to_string()))?;

    // Update instance
    state
        .store
        .update_instance_credentials(&instance.id, &vault_ref, "ready", None)
        .await?;

    // Reload instance
    let instance = state
        .store
        .get_instance_by_id(&instance.id)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    Ok(Json(CredentialsResponse {
        credential_state: instance.credential_state.clone(),
        instance,
    }))
}

/// Check response.
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    /// Check status.
    pub status: String,
    /// Error details if failed.
    pub error: Option<CheckError>,
}

/// Check error details.
#[derive(Debug, Serialize)]
pub struct CheckError {
    /// Error code.
    pub code: String,
    /// Error cause.
    pub cause: String,
    /// Suggested fix.
    pub suggested_fix: Option<String>,
}

/// POST /connect/v1/instances/:name/check
pub async fn check<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<Json<CheckResponse>, ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    let status = state
        .runtime
        .check(&instance.type_id, &instance.config_json)
        .await?;

    match status.status {
        crate::protocol::ConnectionStatusEnum::Succeeded => Ok(Json(CheckResponse {
            status: "ok".to_string(),
            error: None,
        })),
        crate::protocol::ConnectionStatusEnum::Failed => Ok(Json(CheckResponse {
            status: "failed".to_string(),
            error: Some(CheckError {
                code: "check_failed".to_string(),
                cause: status.message.unwrap_or_default(),
                suggested_fix: None,
            }),
        })),
    }
}

/// Discover response.
#[derive(Debug, Serialize)]
pub struct DiscoverResponse {
    /// Discovered catalog.
    pub catalog: Vec<crate::descriptor::StreamDescriptor>,
    /// Discovery timestamp.
    pub discovered_at: chrono::DateTime<chrono::Utc>,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// POST /connect/v1/instances/:name/discover
pub async fn discover<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<Json<DiscoverResponse>, ConnectError> {
    let start = std::time::Instant::now();
    
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    let catalog = state
        .runtime
        .discover(&instance.type_id, &instance.config_json)
        .await?;

    Ok(Json(DiscoverResponse {
        catalog: catalog.streams,
        discovered_at: chrono::Utc::now(),
        duration_ms: start.elapsed().as_millis() as u64,
    }))
}
