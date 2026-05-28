//! RPC route handlers.
//!
//! POST /data/v1/rpc/{name} - invoke a registered SQL function.

use crate::error::DataError;
use crate::middleware::DataCtx;
use crate::rpc::execute_rpc;
use crate::store::DataStore;
use crate::DataState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde_json::Value;

/// POST /data/v1/rpc/{name}
///
/// Invoke a registered RPC function with JSON arguments.
#[utoipa::path(
    post,
    path = "/data/v1/rpc/{name}",
    tag = "data.rpc",
    params(
        ("name" = String, Path, description = "RPC function name")
    ),
    responses(
        (status = 200, description = "Function result"),
        (status = 400, description = "Invalid arguments"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden by policy"),
        (status = 404, description = "Function not found"),
    )
)]
pub async fn post_rpc<S: DataStore + Clone + 'static>(
    State(state): State<DataState<S>>,
    Extension(ctx): Extension<DataCtx>,
    Path(name): Path<String>,
    body: Option<Json<Value>>,
) -> Result<impl IntoResponse, DataError> {
    let args = body.map(|Json(v)| v).unwrap_or(Value::Null);

    let result = execute_rpc(&state, &ctx, &name, args).await?;

    // Return the result
    let body = if result.rows.is_empty() {
        Value::Null
    } else if result.rows_returned == 1 {
        // Single row - return as object
        serde_json::to_value(&result.rows[0]).unwrap_or(Value::Null)
    } else {
        // Multiple rows - return as array
        serde_json::to_value(&result.rows).unwrap_or(Value::Array(vec![]))
    };

    Ok((StatusCode::OK, Json(body)))
}
