//! CRUD route handlers for PostgREST-style API.
//!
//! Handles GET/POST/PATCH/DELETE /data/v1/{table}.

use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::error::DataError;
use crate::execute::{execute_delete, execute_insert, execute_select, execute_update};
use crate::middleware::DataCtx;
use crate::query::{parse_query, Prefer, ReturnMode};
use crate::store::DataStore;
use crate::DataState;

/// GET /data/v1/{table} — Select rows.
#[utoipa::path(
    get,
    path = "/data/v1/{table}",
    tag = "data",
    params(
        ("table" = String, Path, description = "Table name")
    ),
    responses(
        (status = 200, description = "Rows matching the query"),
        (status = 400, description = "Invalid query parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden by policy"),
        (status = 404, description = "Table not found"),
    )
)]
pub async fn get_table<S: DataStore + Clone>(
    State(state): State<DataState<S>>,
    Path(table): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Extension(ctx): Extension<DataCtx>,
) -> Result<Response, DataError> {
    // Parse query
    let plan = parse_query(&params, &headers, &state.config)?;

    // Execute
    let result = execute_select(&state, &ctx, &table, &plan).await?;

    // Build response
    let mut response = Json(result.rows).into_response();

    // Add Content-Range header
    if let Some(total) = result.total_count {
        let range = format!(
            "{}-{}/{}",
            plan.pagination.offset,
            plan.pagination.offset + result.rows_returned.saturating_sub(1),
            total
        );
        response
            .headers_mut()
            .insert("content-range", range.parse().unwrap());
    }

    Ok(response)
}

/// POST /data/v1/{table} — Insert rows.
#[utoipa::path(
    post,
    path = "/data/v1/{table}",
    tag = "data",
    params(
        ("table" = String, Path, description = "Table name")
    ),
    responses(
        (status = 201, description = "Rows inserted"),
        (status = 400, description = "Invalid request body"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden by policy"),
        (status = 404, description = "Table not found"),
    )
)]
pub async fn post_table<S: DataStore + Clone>(
    State(state): State<DataState<S>>,
    Path(table): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Extension(ctx): Extension<DataCtx>,
    Json(body): Json<Value>,
) -> Result<Response, DataError> {
    // Parse prefer header
    let prefer = match headers.get("prefer") {
        Some(v) => match v.to_str() {
            Ok(s) => crate::query::parse_prefer(s)?,
            Err(_) => Prefer::default(),
        },
        None => Prefer::default(),
    };

    // Parse query (for select columns on return)
    let plan = parse_query(&params, &headers, &state.config)?;

    // Execute
    let result = execute_insert(&state, &ctx, &table, body, &prefer, &plan).await?;

    // Build response based on Prefer header
    match prefer.return_mode {
        ReturnMode::Representation => {
            let mut response = Json(result.rows).into_response();
            *response.status_mut() = StatusCode::CREATED;
            Ok(response)
        }
        ReturnMode::Minimal | ReturnMode::HeadersOnly => {
            let mut response = StatusCode::CREATED.into_response();
            if let Some(count) = result.affected_rows {
                response
                    .headers_mut()
                    .insert("x-affected-rows", count.to_string().parse().unwrap());
            }
            Ok(response)
        }
    }
}

/// PATCH /data/v1/{table} — Update rows.
#[utoipa::path(
    patch,
    path = "/data/v1/{table}",
    tag = "data",
    params(
        ("table" = String, Path, description = "Table name")
    ),
    responses(
        (status = 200, description = "Rows updated (with return=representation)"),
        (status = 204, description = "Rows updated (minimal)"),
        (status = 400, description = "Invalid request body"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden by policy"),
        (status = 404, description = "Table not found"),
    )
)]
pub async fn patch_table<S: DataStore + Clone>(
    State(state): State<DataState<S>>,
    Path(table): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Extension(ctx): Extension<DataCtx>,
    Json(body): Json<Value>,
) -> Result<Response, DataError> {
    // Parse prefer header
    let prefer = match headers.get("prefer") {
        Some(v) => match v.to_str() {
            Ok(s) => crate::query::parse_prefer(s)?,
            Err(_) => Prefer::default(),
        },
        None => Prefer::default(),
    };

    // Parse query (for filters)
    let plan = parse_query(&params, &headers, &state.config)?;

    // Execute
    let result = execute_update(&state, &ctx, &table, body, &prefer, &plan).await?;

    // Build response
    match prefer.return_mode {
        ReturnMode::Representation => Ok(Json(result.rows).into_response()),
        ReturnMode::Minimal | ReturnMode::HeadersOnly => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            if let Some(count) = result.affected_rows {
                response
                    .headers_mut()
                    .insert("x-affected-rows", count.to_string().parse().unwrap());
            }
            Ok(response)
        }
    }
}

/// DELETE /data/v1/{table} — Delete rows.
#[utoipa::path(
    delete,
    path = "/data/v1/{table}",
    tag = "data",
    params(
        ("table" = String, Path, description = "Table name")
    ),
    responses(
        (status = 200, description = "Rows deleted (with return=representation)"),
        (status = 204, description = "Rows deleted (minimal)"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden by policy"),
        (status = 404, description = "Table not found"),
    )
)]
pub async fn delete_table<S: DataStore + Clone>(
    State(state): State<DataState<S>>,
    Path(table): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Extension(ctx): Extension<DataCtx>,
) -> Result<Response, DataError> {
    // Parse prefer header
    let prefer = match headers.get("prefer") {
        Some(v) => match v.to_str() {
            Ok(s) => crate::query::parse_prefer(s)?,
            Err(_) => Prefer::default(),
        },
        None => Prefer::default(),
    };

    // Parse query (for filters)
    let plan = parse_query(&params, &headers, &state.config)?;

    // Execute
    let result = execute_delete(&state, &ctx, &table, &prefer, &plan).await?;

    // Build response
    match prefer.return_mode {
        ReturnMode::Representation => Ok(Json(result.rows).into_response()),
        ReturnMode::Minimal | ReturnMode::HeadersOnly => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            if let Some(count) = result.affected_rows {
                response
                    .headers_mut()
                    .insert("x-affected-rows", count.to_string().parse().unwrap());
            }
            Ok(response)
        }
    }
}
