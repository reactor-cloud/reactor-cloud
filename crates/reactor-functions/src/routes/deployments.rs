//! Deployment routes for bundle upload and management.

use axum::{
    extract::{Extension, Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::io::Cursor;

use crate::{
    bundle::{
        compute_sha256, extract_manifest, validate_bundle_structure, bundle_object_key, RuntimeKind,
        BUNDLE_MAX_SIZE, SYSTEM_BUCKET,
    },
    error::FunctionsError,
    state::{FunctionCtx, FunctionsState},
    store::{DeploymentCreate, DeploymentStatus, FunctionsStore, PgFunctionsStore},
};

/// Response for a deployment.
#[derive(Debug, Serialize)]
pub struct DeploymentResponse {
    /// Deployment ID.
    pub id: String,
    /// Function ID.
    pub function_id: String,
    /// Version number.
    pub version: i64,
    /// Bundle storage bucket.
    pub bundle_bucket: String,
    /// Bundle object key.
    pub bundle_object_key: String,
    /// Bundle SHA256 (hex-encoded).
    pub bundle_sha256: String,
    /// Bundle size in bytes.
    pub bundle_size: i64,
    /// Deployment status.
    pub status: String,
    /// Status detail (error message if failed).
    pub status_detail: Option<String>,
    /// Runtime reference (e.g., Lambda ARN).
    pub runtime_ref: Option<String>,
    /// Deployed timestamp.
    pub deployed_at: String,
    /// Whether this deployment has job configuration.
    pub is_job: bool,
}

impl DeploymentResponse {
    /// Create a deployment response from a store deployment record.
    pub fn from_deployment(d: crate::store::Deployment) -> Self {
        // Check if manifest has job config
        let is_job = d
            .manifest_json
            .get("job")
            .map(|v| !v.is_null())
            .unwrap_or(false);

        Self {
            id: d.id.to_string(),
            function_id: d.function_id.to_string(),
            version: d.version,
            bundle_bucket: d.bundle_bucket,
            bundle_object_key: d.bundle_object_key,
            bundle_sha256: hex::encode(&d.bundle_sha256),
            bundle_size: d.bundle_size,
            status: d.status,
            status_detail: d.status_detail,
            runtime_ref: d.runtime_ref,
            deployed_at: d.deployed_at.to_rfc3339(),
            is_job,
        }
    }
}

impl From<crate::store::Deployment> for DeploymentResponse {
    fn from(d: crate::store::Deployment) -> Self {
        DeploymentResponse::from_deployment(d)
    }
}

/// Response for listing deployments.
#[derive(Debug, Serialize)]
pub struct ListDeploymentsResponse {
    /// List of deployments.
    pub deployments: Vec<DeploymentResponse>,
}

/// POST /fn/v1/_admin/functions/:name/deployments
///
/// Upload a bundle and create a new deployment.
/// The bundle is a multipart form with:
/// - `bundle`: the zip file
/// - `sha256`: hex-encoded SHA256 hash of the bundle
pub async fn create_deployment(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(function_name): Path<String>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:deploy", function_name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("*") {
        return Err(FunctionsError::PermissionDenied(format!(
            "{} permission required",
            permission
        )));
    }

    let store = PgFunctionsStore::new(state.pool.clone());

    // Get the function
    let function = store
        .get_function_by_name(ctx.active_org(), &function_name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(function_name.clone()))?;

    // Parse multipart form
    let mut bundle_data: Option<Vec<u8>> = None;
    let mut expected_sha256: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| FunctionsError::InvalidRequest(format!("multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "bundle" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| FunctionsError::InvalidRequest(format!("failed to read bundle: {}", e)))?;

                // Check size
                if data.len() as u64 > BUNDLE_MAX_SIZE {
                    return Err(FunctionsError::BundleTooLarge {
                        size: data.len() as u64,
                        limit: BUNDLE_MAX_SIZE,
                    });
                }

                bundle_data = Some(data.to_vec());
            }
            "sha256" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| FunctionsError::InvalidRequest(format!("failed to read sha256: {}", e)))?;
                expected_sha256 = Some(text);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate we have both parts
    let bundle_data = bundle_data
        .ok_or_else(|| FunctionsError::InvalidRequest("bundle field required".to_string()))?;
    let expected_sha256 = expected_sha256
        .ok_or_else(|| FunctionsError::InvalidRequest("sha256 field required".to_string()))?;

    // Verify SHA256
    let expected_hash = hex::decode(&expected_sha256)
        .map_err(|_| FunctionsError::InvalidRequest("invalid sha256 hex encoding".to_string()))?;

    let actual_hash = compute_sha256(&bundle_data);
    if actual_hash != expected_hash {
        return Err(FunctionsError::BundleInvalid(
            "SHA256 hash mismatch".to_string(),
        ));
    }

    // Validate bundle structure
    let cursor = Cursor::new(&bundle_data);
    validate_bundle_structure(cursor)?;

    // Extract and validate manifest
    let cursor = Cursor::new(&bundle_data);
    let manifest = extract_manifest(cursor)?;
    manifest.validate(&function.runtime)?;

    // Check that manifest name matches function name
    if manifest.name != function.name {
        return Err(FunctionsError::ManifestInvalid(format!(
            "manifest name '{}' doesn't match function name '{}'",
            manifest.name, function.name
        )));
    }

    // TODO: Check that env_keys and secret_keys exist in the env table

    // Get next version
    let version = store.next_deployment_version(function.id).await?;

    // Generate storage key
    let object_key = bundle_object_key(&function.name, version);

    // TODO: Upload to reactor-storage
    // For now, we'll skip the actual storage upload and just create the deployment record
    tracing::warn!(
        function = %function.name,
        version = version,
        object_key = %object_key,
        "TODO: upload bundle to reactor-storage"
    );

    // Create deployment record (starts in pending status)
    let deployment = store
        .create_deployment(DeploymentCreate {
            function_id: function.id,
            bundle_bucket: SYSTEM_BUCKET.to_string(),
            bundle_object_key: object_key.clone(),
            bundle_sha256: actual_hash,
            bundle_size: bundle_data.len() as i64,
            manifest_json: serde_json::to_value(&manifest)
                .map_err(|e| FunctionsError::Internal(format!("failed to serialize manifest: {}", e)))?,
            deployed_by_user_id: ctx.user_id().map(|id| id.into()),
        })
        .await?;

    // Deploy to runtime and flip status
    let runtime_kind: RuntimeKind = function.runtime.parse()
        .map_err(|_| FunctionsError::Internal(format!("invalid runtime: {}", function.runtime)))?;
    
    let runtime_deploy_result = if let Some(runtime) = state.runtimes.get(runtime_kind).await {
        // Write bundle to staging directory in workdir
        let staging_dir = std::path::PathBuf::from(&state.config.workdir)
            .join("staging")
            .join(deployment.id.to_string());
        tokio::fs::create_dir_all(&staging_dir)
            .await
            .map_err(|e| FunctionsError::Internal(format!("failed to create staging dir: {}", e)))?;
        let bundle_path = staging_dir.join(format!("{}.fnpkg.zip", function.name));
        tokio::fs::write(&bundle_path, &bundle_data)
            .await
            .map_err(|e| FunctionsError::Internal(format!("failed to write bundle: {}", e)))?;

        // Deploy to runtime
        let result = match runtime.deploy(deployment.id, &function.name, &manifest, &bundle_path).await {
            Ok(handle) => {
                tracing::info!(
                    function = %function.name,
                    deployment_id = %deployment.id,
                    version = version,
                    runtime_ref = ?handle.runtime_ref,
                    "runtime deployment succeeded"
                );
                Ok(handle.runtime_ref)
            }
            Err(e) => {
                tracing::error!(
                    function = %function.name,
                    deployment_id = %deployment.id,
                    error = %e,
                    "runtime deployment failed"
                );
                Err(e.to_string())
            }
        };

        // Cleanup staging directory (bundle has been extracted by runtime)
        if let Err(e) = tokio::fs::remove_dir_all(&staging_dir).await {
            tracing::warn!(staging_dir = %staging_dir.display(), error = %e, "failed to cleanup staging dir");
        }

        result
    } else {
        // No runtime registered for this kind - mark as ready anyway for testing
        tracing::warn!(
            function = %function.name,
            runtime = %function.runtime,
            "no runtime registered for this kind, marking ready"
        );
        Ok(None)
    };

    // Update deployment status based on runtime result
    match runtime_deploy_result {
        Ok(runtime_ref) => {
            store
                .update_deployment_status(
                    deployment.id,
                    DeploymentStatus::Ready,
                    None,
                    runtime_ref,
                )
                .await?;
        }
        Err(error_detail) => {
            store
                .update_deployment_status(
                    deployment.id,
                    DeploymentStatus::Failed,
                    Some(error_detail),
                    None,
                )
                .await?;
            return Err(FunctionsError::RuntimeError(
                "deployment failed - see status_detail".to_string()
            ));
        }
    }

    // Reload deployment to get updated status
    let deployment = store
        .get_deployment(deployment.id)
        .await?
        .ok_or_else(|| FunctionsError::DeploymentNotFound(deployment.id.to_string()))?;

    // TODO: PR 14 - Record audit event

    // Job integration: if the manifest has job configuration, reactor-jobs should:
    // 1. Create/update the job record linking to this function
    // 2. Create triggers from manifest.job.triggers
    // 3. Apply retry config and concurrency limits
    //
    // This is handled by reactor-jobs-server listening for deployment events
    // or by reactor-jobs polling for functions with job configs.
    if manifest.is_job() {
        tracing::info!(
            function = %function.name,
            version = version,
            triggers = ?manifest.job.as_ref().map(|j| j.triggers.len()),
            "deployment has job configuration - reactor-jobs will create/update job record"
        );
    }

    Ok((StatusCode::CREATED, Json(DeploymentResponse::from(deployment))))
}

/// GET /fn/v1/_admin/functions/:name/deployments
///
/// List all deployments for a function.
pub async fn list_deployments(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(function_name): Path<String>,
) -> Result<impl IntoResponse, FunctionsError> {
    let store = PgFunctionsStore::new(state.pool.clone());

    // Get the function
    let function = store
        .get_function_by_name(ctx.active_org(), &function_name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(function_name))?;

    // List deployments
    let deployments = store.list_deployments(function.id).await?;

    Ok(Json(ListDeploymentsResponse {
        deployments: deployments.into_iter().map(DeploymentResponse::from).collect(),
    }))
}

/// GET /fn/v1/_admin/functions/:name/deployments/:deployment_id
///
/// Get a single deployment.
pub async fn get_deployment(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path((function_name, deployment_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, FunctionsError> {
    let store = PgFunctionsStore::new(state.pool.clone());

    // Get the function first to verify ownership
    let function = store
        .get_function_by_name(ctx.active_org(), &function_name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(function_name))?;

    // Parse deployment ID
    let deployment_uuid = deployment_id
        .parse()
        .map_err(|_| FunctionsError::DeploymentNotFound(deployment_id.clone()))?;

    // Get deployment
    let deployment = store
        .get_deployment(deployment_uuid)
        .await?
        .ok_or_else(|| FunctionsError::DeploymentNotFound(deployment_id))?;

    // Verify deployment belongs to this function
    if deployment.function_id != function.id {
        return Err(FunctionsError::DeploymentNotFound(
            deployment.id.to_string(),
        ));
    }

    Ok(Json(DeploymentResponse::from(deployment)))
}

/// Response for promote/rollback operations.
#[derive(Debug, Serialize)]
pub struct PromoteResponse {
    /// Previous deployment ID (if any).
    pub previous_deployment_id: Option<String>,
    /// New current deployment ID.
    pub current_deployment_id: String,
    /// Message.
    pub message: String,
}

/// POST /fn/v1/_admin/functions/:name/promote
///
/// Promote a deployment to be the current version.
/// Body: { "deployment_id": "uuid" }
pub async fn promote_deployment(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(function_name): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:admin", function_name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
        return Err(FunctionsError::PermissionDenied(permission));
    }

    let store = PgFunctionsStore::new(state.pool.clone());

    // Get the function
    let function = store
        .get_function_by_name(ctx.active_org(), &function_name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(function_name.clone()))?;

    // Parse deployment ID from body
    let deployment_id_str = body
        .get("deployment_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| FunctionsError::InvalidRequest("deployment_id is required".to_string()))?;

    let deployment_id: uuid::Uuid = deployment_id_str
        .parse()
        .map_err(|_| FunctionsError::InvalidRequest("invalid deployment_id format".to_string()))?;

    // Get the deployment and verify it belongs to this function
    let deployment = store
        .get_deployment(deployment_id)
        .await?
        .ok_or_else(|| FunctionsError::DeploymentNotFound(deployment_id_str.to_string()))?;

    if deployment.function_id != function.id {
        return Err(FunctionsError::DeploymentNotFound(deployment_id_str.to_string()));
    }

    // Verify deployment is ready
    if deployment.status != "ready" {
        return Err(FunctionsError::InvalidRequest(format!(
            "deployment status is '{}', must be 'ready' to promote",
            deployment.status
        )));
    }

    // Get previous deployment ID for response
    let previous_deployment_id = function.current_deployment_id.map(|id| id.to_string());

    // Atomic swap of current_deployment_id
    store
        .set_current_deployment(function.id, Some(deployment_id))
        .await?;

    // TODO: PR 14 - Record audit event

    tracing::info!(
        function = %function.name,
        deployment_id = %deployment_id,
        previous = ?previous_deployment_id,
        "promoted deployment"
    );

    Ok(Json(PromoteResponse {
        previous_deployment_id,
        current_deployment_id: deployment_id.to_string(),
        message: format!("Deployment {} is now serving traffic", deployment_id),
    }))
}

/// POST /fn/v1/_admin/functions/:name/rollback
///
/// Rollback to the previous deployment.
/// Optional body: { "deployment_id": "uuid" } to rollback to a specific version.
pub async fn rollback_deployment(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(function_name): Path<String>,
    body: Option<Json<serde_json::Value>>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:admin", function_name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
        return Err(FunctionsError::PermissionDenied(permission));
    }

    let store = PgFunctionsStore::new(state.pool.clone());

    // Get the function
    let function = store
        .get_function_by_name(ctx.active_org(), &function_name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(function_name.clone()))?;

    // Get target deployment ID
    let target_deployment_id = if let Some(Json(body)) = body {
        if let Some(deployment_id_str) = body.get("deployment_id").and_then(|v| v.as_str()) {
            // Rollback to specific deployment
            let deployment_id: uuid::Uuid = deployment_id_str
                .parse()
                .map_err(|_| FunctionsError::InvalidRequest("invalid deployment_id format".to_string()))?;
            Some(deployment_id)
        } else {
            None
        }
    } else {
        None
    };

    let target_deployment_id = match target_deployment_id {
        Some(id) => id,
        None => {
            // Find the previous deployment (second most recent ready deployment)
            let deployments = store.list_deployments(function.id).await?;
            let ready_deployments: Vec<_> = deployments
                .into_iter()
                .filter(|d| d.status == "ready")
                .collect();

            // Find the deployment before the current one
            let current_id = function.current_deployment_id;
            let previous = ready_deployments
                .into_iter()
                .skip_while(|d| Some(d.id) == current_id)
                .next()
                .ok_or_else(|| {
                    FunctionsError::InvalidRequest("no previous deployment to rollback to".to_string())
                })?;

            previous.id
        }
    };

    // Verify the target deployment belongs to this function and is ready
    let deployment = store
        .get_deployment(target_deployment_id)
        .await?
        .ok_or_else(|| FunctionsError::DeploymentNotFound(target_deployment_id.to_string()))?;

    if deployment.function_id != function.id {
        return Err(FunctionsError::DeploymentNotFound(target_deployment_id.to_string()));
    }

    if deployment.status != "ready" {
        return Err(FunctionsError::InvalidRequest(format!(
            "deployment status is '{}', must be 'ready' to rollback to",
            deployment.status
        )));
    }

    // Get previous (current) deployment ID for response
    let previous_deployment_id = function.current_deployment_id.map(|id| id.to_string());

    // Atomic swap of current_deployment_id
    store
        .set_current_deployment(function.id, Some(target_deployment_id))
        .await?;

    // TODO: PR 14 - Record audit event

    tracing::info!(
        function = %function.name,
        deployment_id = %target_deployment_id,
        previous = ?previous_deployment_id,
        "rolled back deployment"
    );

    Ok(Json(PromoteResponse {
        previous_deployment_id,
        current_deployment_id: target_deployment_id.to_string(),
        message: format!(
            "Rolled back to deployment {} (version {})",
            target_deployment_id, deployment.version
        ),
    }))
}
