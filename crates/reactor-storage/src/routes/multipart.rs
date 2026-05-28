//! Multipart upload routes.
//!
//! S3-style multipart uploads for large files.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::StorageError;
use crate::middleware::StorageCtx;
use crate::state::StorageState;
use crate::store::{BlobStore, MetadataStore, PgMetadataStore};

#[cfg(feature = "fs")]
use crate::fs::FsBlobStore;

/// Request to initiate multipart upload.
#[derive(Debug, Deserialize)]
pub struct CreateMultipartRequest {
    /// Content type for the final object.
    pub content_type: Option<String>,
}

/// Response for initiated multipart upload.
#[derive(Debug, Serialize)]
pub struct CreateMultipartResponse {
    /// Upload ID for subsequent part uploads.
    pub upload_id: String,
}

/// Query parameters for part upload.
#[derive(Debug, Deserialize)]
pub struct UploadPartQuery {
    /// Upload ID from initiate response.
    pub upload_id: String,
    /// Part number (1-10000).
    pub part_number: i32,
}

/// Response for uploaded part.
#[derive(Debug, Serialize)]
pub struct UploadPartResponse {
    /// ETag for the uploaded part.
    pub etag: String,
}

/// Part info for completing multipart upload.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PartInfo {
    /// Part number.
    pub part_number: i32,
    /// ETag from upload_part response.
    pub etag: String,
}

/// Request to complete multipart upload.
#[derive(Debug, Deserialize)]
pub struct CompleteMultipartRequest {
    /// List of parts in order.
    pub parts: Vec<PartInfo>,
}

/// Query for complete/abort operations.
#[derive(Debug, Deserialize)]
pub struct MultipartQuery {
    /// Upload ID.
    pub upload_id: String,
}

/// Response for completed upload.
#[derive(Debug, Serialize)]
pub struct CompleteMultipartResponse {
    /// ETag for the completed object.
    pub etag: String,
    /// Object key.
    pub key: String,
}

/// Helper to get blob store from state.
#[cfg(any(feature = "fs", feature = "s3"))]
fn get_blob_store(state: &StorageState) -> Result<Arc<dyn BlobStore>, StorageError> {
    // If a blob store is pre-initialized (e.g., S3), use it
    if let Some(ref store) = state.blob_store {
        return Ok(store.clone());
    }

    // Fall back to FS blob store if fs_base_path is configured
    #[cfg(feature = "fs")]
    if let Some(ref base_path) = state.config.fs_base_path {
        let mut store = FsBlobStore::new(base_path);
        if let Some(ref secret) = state.config.signing_secret {
            store = store.with_signing_secret(secret);
        }
        return Ok(Arc::new(store));
    }

    // No blob store available
    Err(StorageError::Internal(
        "No blob store configured - set blob_store in state or fs_base_path in config".into(),
    ))
}

#[cfg(not(any(feature = "fs", feature = "s3")))]
fn get_blob_store(_state: &StorageState) -> Result<Arc<dyn BlobStore>, StorageError> {
    Err(StorageError::Internal(
        "No blob store configured".into(),
    ))
}

/// POST /storage/v1/object/:bucket/:key?uploads
///
/// Initiate a multipart upload.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn create_multipart_upload(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
    Json(request): Json<CreateMultipartRequest>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let metadata_store = PgMetadataStore::new(state.pool.clone());

    // Get bucket
    let bucket = metadata_store
        .get_bucket_by_slug(org_id.into_uuid(), &bucket_ref)
        .await?
        .ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Check write permission
    let write_perm = format!("storage:{}:write", bucket.slug);
    if !ctx.has_permission(&write_perm) && !ctx.has_permission("storage:*:write") {
        return Err(StorageError::PermissionDenied(format!(
            "{} permission required",
            write_perm
        )));
    }

    // Create the multipart upload in blob store
    let blob_store = get_blob_store(&state)?;
    let org_id_str = org_id.to_string();
    let upload_id = blob_store
        .create_multipart(&org_id_str, &bucket.slug, &key, request.content_type.as_deref())
        .await?;

    // Record in metadata store
    let user_id = ctx.user_id().map(|u| u.into_uuid());
    metadata_store
        .create_multipart_upload(
            bucket.id,
            &key,
            &upload_id,
            user_id,
            request.content_type.as_deref(),
        )
        .await?;

    Ok(Json(CreateMultipartResponse { upload_id }))
}

/// PUT /storage/v1/object/:bucket/:key?uploadId=X&partNumber=N
///
/// Upload a part.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn upload_part(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
    Query(query): Query<UploadPartQuery>,
    body: Body,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let metadata_store = PgMetadataStore::new(state.pool.clone());

    // Get bucket
    let bucket = metadata_store
        .get_bucket_by_slug(org_id.into_uuid(), &bucket_ref)
        .await?
        .ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Check write permission
    let write_perm = format!("storage:{}:write", bucket.slug);
    if !ctx.has_permission(&write_perm) && !ctx.has_permission("storage:*:write") {
        return Err(StorageError::PermissionDenied(format!(
            "{} permission required",
            write_perm
        )));
    }

    // Verify the upload exists
    let _upload = metadata_store
        .get_multipart_upload(bucket.id, &query.upload_id)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("Upload {} not found", query.upload_id)))?;

    // Collect body bytes
    let body_bytes = axum::body::to_bytes(body, state.config.max_upload_size as usize)
        .await
        .map_err(|e| {
            if e.to_string().contains("length limit") {
                StorageError::TooLarge {
                    size: 0,
                    limit: state.config.max_upload_size,
                }
            } else {
                StorageError::InvalidRequest(e.to_string())
            }
        })?;

    // Upload the part
    let blob_store = get_blob_store(&state)?;
    let org_id_str = org_id.to_string();
    let etag = blob_store
        .upload_part(
            &org_id_str,
            &bucket.slug,
            &key,
            &query.upload_id,
            query.part_number,
            body_bytes.clone(),
        )
        .await?;

    // Record the part in metadata
    metadata_store
        .add_multipart_part(
            bucket.id,
            &query.upload_id,
            query.part_number,
            &etag,
            body_bytes.len() as i64,
        )
        .await?;

    Ok((
        StatusCode::OK,
        [(header::ETAG, etag.clone())],
        Json(UploadPartResponse { etag }),
    ))
}

/// POST /storage/v1/object/:bucket/:key?uploadId=X
///
/// Complete a multipart upload.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn complete_multipart_upload(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
    Query(query): Query<MultipartQuery>,
    Json(request): Json<CompleteMultipartRequest>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let metadata_store = PgMetadataStore::new(state.pool.clone());

    // Get bucket
    let bucket = metadata_store
        .get_bucket_by_slug(org_id.into_uuid(), &bucket_ref)
        .await?
        .ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Check write permission
    let write_perm = format!("storage:{}:write", bucket.slug);
    if !ctx.has_permission(&write_perm) && !ctx.has_permission("storage:*:write") {
        return Err(StorageError::PermissionDenied(format!(
            "{} permission required",
            write_perm
        )));
    }

    // Verify the upload exists
    let upload = metadata_store
        .get_multipart_upload(bucket.id, &query.upload_id)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("Upload {} not found", query.upload_id)))?;

    // Convert parts to tuples
    let parts: Vec<(i32, String)> = request
        .parts
        .iter()
        .map(|p| (p.part_number, p.etag.clone()))
        .collect();

    // Complete in blob store
    let blob_store = get_blob_store(&state)?;
    let org_id_str = org_id.to_string();
    let etag = blob_store
        .complete_multipart(&org_id_str, &bucket.slug, &key, &query.upload_id, &parts)
        .await?;

    // Get total size from parts
    let stored_parts = metadata_store
        .list_multipart_parts(bucket.id, &query.upload_id)
        .await?;
    let total_size: i64 = stored_parts.iter().map(|p| p.size).sum();

    // Create the object metadata
    use crate::store::ObjectCreate;
    let _object = metadata_store
        .upsert_object(ObjectCreate {
            bucket_id: bucket.id,
            key: key.clone(),
            content_type: upload.content_type,
            content_length: total_size,
            etag: Some(etag.clone()),
            metadata: serde_json::json!({}),
            created_by: ctx.user_id().map(|u| u.into_uuid()),
        })
        .await?;

    // Clean up multipart records
    metadata_store
        .delete_multipart_upload(bucket.id, &query.upload_id)
        .await?;

    Ok(Json(CompleteMultipartResponse { etag, key }))
}

/// DELETE /storage/v1/object/:bucket/:key?uploadId=X
///
/// Abort a multipart upload.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn abort_multipart_upload(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
    Query(query): Query<MultipartQuery>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let metadata_store = PgMetadataStore::new(state.pool.clone());

    // Get bucket
    let bucket = metadata_store
        .get_bucket_by_slug(org_id.into_uuid(), &bucket_ref)
        .await?
        .ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Check write permission
    let write_perm = format!("storage:{}:write", bucket.slug);
    if !ctx.has_permission(&write_perm) && !ctx.has_permission("storage:*:write") {
        return Err(StorageError::PermissionDenied(format!(
            "{} permission required",
            write_perm
        )));
    }

    // Abort in blob store
    let blob_store = get_blob_store(&state)?;
    let org_id_str = org_id.to_string();
    blob_store
        .abort_multipart(&org_id_str, &bucket.slug, &key, &query.upload_id)
        .await?;

    // Clean up metadata
    metadata_store
        .delete_multipart_upload(bucket.id, &query.upload_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
