//! Admin routes for function CRUD.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::FunctionsError,
    state::{FunctionCtx, FunctionsState},
    store::{Function, FunctionCreate, FunctionsStore, PgFunctionsStore},
    FUNCTION_NAME_REGEX,
};

/// Request body for creating a function.
#[derive(Debug, Deserialize)]
pub struct CreateFunctionRequest {
    /// Function name (lowercase alphanumeric with hyphens, 3-63 chars).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Runtime: 'wasm', 'bun', or 'lambda'.
    pub runtime: String,
}

/// Response for a single function.
#[derive(Debug, Serialize)]
pub struct FunctionResponse {
    /// Function ID.
    pub id: String,
    /// Organization ID.
    pub org_id: String,
    /// Function name.
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// Runtime type.
    pub runtime: String,
    /// Current deployment ID.
    pub current_deployment_id: Option<String>,
    /// Created timestamp.
    pub created_at: String,
    /// Updated timestamp.
    pub updated_at: String,
}

impl From<Function> for FunctionResponse {
    fn from(f: Function) -> Self {
        Self {
            id: f.id.to_string(),
            org_id: f.org_id.to_string(),
            name: f.name,
            description: f.description,
            runtime: f.runtime,
            current_deployment_id: f.current_deployment_id.map(|id| id.to_string()),
            created_at: f.created_at.to_rfc3339(),
            updated_at: f.updated_at.to_rfc3339(),
        }
    }
}

/// Response for listing functions.
#[derive(Debug, Serialize)]
pub struct ListFunctionsResponse {
    /// List of functions.
    pub functions: Vec<FunctionResponse>,
}

/// POST /fn/v1/_admin/functions
///
/// Create a new function.
pub async fn create_function(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Json(body): Json<CreateFunctionRequest>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    if !ctx.has_permission("functions:create") && !ctx.has_permission("*") {
        return Err(FunctionsError::PermissionDenied(
            "functions:create permission required".to_string(),
        ));
    }

    // Validate function name
    if !FUNCTION_NAME_REGEX.is_match(&body.name) {
        return Err(FunctionsError::InvalidFunctionName(format!(
            "'{}' must be 3-63 lowercase alphanumeric chars with hyphens, starting and ending with alphanumeric",
            body.name
        )));
    }

    // Validate runtime
    if !matches!(body.runtime.as_str(), "wasm" | "bun" | "lambda") {
        return Err(FunctionsError::UnsupportedRuntime(format!(
            "runtime must be 'wasm', 'bun', or 'lambda', got '{}'",
            body.runtime
        )));
    }

    // Create the function
    let store = PgFunctionsStore::new(state.pool.clone());
    let function = store
        .create_function(FunctionCreate {
            org_id: ctx.active_org(),
            name: body.name,
            description: body.description,
            runtime: body.runtime,
        })
        .await?;

    // TODO: PR 14 - Record audit event

    Ok((StatusCode::CREATED, Json(FunctionResponse::from(function))))
}

/// GET /fn/v1/_admin/functions
///
/// List all functions for the current organization.
pub async fn list_functions(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission (list requires at least being authenticated to the org)
    // No specific permission required for listing - you can only see your org's functions

    let store = PgFunctionsStore::new(state.pool.clone());
    let functions = store.list_functions(ctx.active_org()).await?;

    Ok(Json(ListFunctionsResponse {
        functions: functions.into_iter().map(FunctionResponse::from).collect(),
    }))
}

/// GET /fn/v1/_admin/functions/:name
///
/// Get a single function by name.
pub async fn get_function(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, FunctionsError> {
    let store = PgFunctionsStore::new(state.pool.clone());
    let function = store
        .get_function_by_name(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(name))?;

    Ok(Json(FunctionResponse::from(function)))
}

/// DELETE /fn/v1/_admin/functions/:name
///
/// Delete a function and all its deployments.
pub async fn delete_function(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:admin", name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("*") {
        return Err(FunctionsError::PermissionDenied(format!(
            "{} permission required",
            permission
        )));
    }

    let store = PgFunctionsStore::new(state.pool.clone());

    // Get the function first to ensure it exists and belongs to this org
    let function = store
        .get_function_by_name(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(name.clone()))?;

    // TODO: PR 6 - Destroy runtime resources for all deployments

    // Delete the function (cascades to deployments, env, policies)
    let deleted = store.delete_function(function.id).await?;

    if deleted {
        // TODO: PR 14 - Record audit event
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(FunctionsError::FunctionNotFound(name))
    }
}
