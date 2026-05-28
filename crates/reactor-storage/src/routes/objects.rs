//! Object management routes.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension,
};
use futures::StreamExt;
use serde::Serialize;
use std::sync::Arc;

use crate::error::StorageError;
use crate::middleware::StorageCtx;
use crate::state::StorageState;
use crate::store::{BlobStore, MetadataStore, ObjectCreate, PgMetadataStore};

#[cfg(feature = "fs")]
use crate::fs::FsBlobStore;
#[cfg(all(feature = "s3", not(feature = "fs")))]
use crate::s3::S3BlobStore;

/// Object metadata response.
#[derive(Debug, Serialize)]
pub struct ObjectMetaResponse {
    /// Content length.
    pub content_length: u64,
    /// Content type.
    pub content_type: Option<String>,
    /// ETag.
    pub etag: Option<String>,
}

/// Parse Range header.
/// Format: "bytes=start-end" or "bytes=start-" or "bytes=-suffix"
fn parse_range_header(value: &str, total_size: u64) -> Option<std::ops::Range<u64>> {
    let value = value.strip_prefix("bytes=")?;
    let parts: Vec<&str> = value.split('-').collect();

    if parts.len() != 2 {
        return None;
    }

    let (start, end) = if parts[0].is_empty() {
        // "-suffix" format: last N bytes
        let suffix: u64 = parts[1].parse().ok()?;
        let start = total_size.saturating_sub(suffix);
        (start, total_size)
    } else if parts[1].is_empty() {
        // "start-" format: from start to end
        let start: u64 = parts[0].parse().ok()?;
        (start, total_size)
    } else {
        // "start-end" format
        let start: u64 = parts[0].parse().ok()?;
        let end: u64 = parts[1].parse().ok()?;
        (start, end + 1) // Range header is inclusive, we need exclusive
    };

    if start >= total_size || start >= end {
        return None;
    }

    Some(start..end.min(total_size))
}

/// Helper to get blob store from state.
///
/// Returns the pre-initialized blob store from state, or creates an FS blob store
/// on-demand for FS-only configurations.
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

/// PUT /storage/v1/object/:bucket/:key
///
/// Upload an object.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn put_object(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    // Require org context
    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let metadata_store = PgMetadataStore::new(state.pool.clone());

    // Get bucket
    let bucket = metadata_store
        .get_bucket_by_slug(org_id.into_uuid(), &bucket_ref)
        .await?
        .ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Check permission: storage:{bucket}:write or storage:*:write
    let write_perm = format!("storage:{}:write", bucket.slug);
    if !ctx.has_permission(&write_perm) && !ctx.has_permission("storage:*:write") {
        return Err(StorageError::PermissionDenied(format!(
            "{} permission required",
            write_perm
        )));
    }

    // Get content type from header
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

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

    let content_length = body_bytes.len() as i64;

    // Store the blob
    let blob_store = get_blob_store(&state)?;
    let org_id_str = org_id.to_string();
    let etag = blob_store
        .put(&org_id_str, &bucket.slug, &key, body_bytes, content_type.as_deref())
        .await?;

    // Upsert object metadata
    let _object = metadata_store
        .upsert_object(ObjectCreate {
            bucket_id: bucket.id,
            key: key.clone(),
            content_type,
            content_length,
            etag: Some(etag.clone()),
            metadata: serde_json::json!({}),
            created_by: ctx.user_id().map(|u| u.into_uuid()),
        })
        .await?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::ETAG, &etag);

    Ok(response.body(Body::empty()).unwrap())
}

/// GET /storage/v1/object/:bucket/:key
///
/// Download an object with optional Range support.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn get_object(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StorageError> {
    let metadata_store = PgMetadataStore::new(state.pool.clone());

    // For anonymous requests, we need to verify the bucket is public
    let bucket = if ctx.is_anonymous {
        // Try to find the bucket by slug across all orgs (public buckets)
        // This is a simplified approach - in production you'd want a more specific query
        return Err(StorageError::AuthRequired);
    } else {
        let org_id = ctx
            .org_id()
            .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

        metadata_store
            .get_bucket_by_slug(org_id.into_uuid(), &bucket_ref)
            .await?
            .ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?
    };

    // Check permission for non-public buckets
    if !bucket.is_public {
        let read_perm = format!("storage:{}:read", bucket.slug);
        if !ctx.has_permission(&read_perm) && !ctx.has_permission("storage:*:read") {
            return Err(StorageError::PermissionDenied(format!(
                "{} permission required",
                read_perm
            )));
        }
    }

    // Get object metadata first to check size
    let org_id = ctx.org_id().unwrap();
    let org_id_str = org_id.to_string();
    let blob_store = get_blob_store(&state)?;

    // Parse Range header if present
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|range_str| {
            // We need the total size first - get it from head
            // For simplicity, we'll parse after getting metadata
            Some(range_str.to_string())
        });

    // Get the object
    let (stream, meta) = blob_store.get(&org_id_str, &bucket.slug, &key, None).await?;

    // Now parse range if present
    let range = range.and_then(|r| parse_range_header(&r, meta.content_length));

    // If range requested, we need to re-fetch with the range
    let (stream, status, content_range) = if let Some(ref r) = range {
        let (ranged_stream, _) = blob_store
            .get(&org_id_str, &bucket.slug, &key, Some(r.clone()))
            .await?;
        let content_range = format!(
            "bytes {}-{}/{}",
            r.start,
            r.end - 1,
            meta.content_length
        );
        (ranged_stream, StatusCode::PARTIAL_CONTENT, Some(content_range))
    } else {
        (stream, StatusCode::OK, None)
    };

    // Build response
    let mut response = Response::builder().status(status);

    if let Some(ct) = meta.content_type {
        response = response.header(header::CONTENT_TYPE, ct);
    }

    if let Some(etag) = meta.etag {
        response = response.header(header::ETAG, etag);
    }

    response = response.header(header::ACCEPT_RANGES, "bytes");

    if let Some(cr) = content_range {
        let content_length = range.as_ref().map(|r| r.end - r.start).unwrap_or(meta.content_length);
        response = response
            .header(header::CONTENT_RANGE, cr)
            .header(header::CONTENT_LENGTH, content_length);
    } else {
        response = response.header(header::CONTENT_LENGTH, meta.content_length);
    }

    // Convert stream to body
    let body_stream = stream.map(|result| {
        result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    });

    Ok(response.body(Body::from_stream(body_stream)).unwrap())
}

/// HEAD /storage/v1/object/:bucket/:key
///
/// Get object metadata without downloading.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn head_object(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
) -> Result<Response, StorageError> {
    // Similar auth check as get_object
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let metadata_store = PgMetadataStore::new(state.pool.clone());
    let bucket = metadata_store
        .get_bucket_by_slug(org_id.into_uuid(), &bucket_ref)
        .await?
        .ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Check permission
    if !bucket.is_public {
        let read_perm = format!("storage:{}:read", bucket.slug);
        if !ctx.has_permission(&read_perm) && !ctx.has_permission("storage:*:read") {
            return Err(StorageError::PermissionDenied(format!(
                "{} permission required",
                read_perm
            )));
        }
    }

    let blob_store = get_blob_store(&state)?;
    let org_id_str = org_id.to_string();
    let meta = blob_store.head(&org_id_str, &bucket.slug, &key).await?;

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_LENGTH, meta.content_length)
        .header(header::ACCEPT_RANGES, "bytes");

    if let Some(ct) = meta.content_type {
        response = response.header(header::CONTENT_TYPE, ct);
    }

    if let Some(etag) = meta.etag {
        response = response.header(header::ETAG, etag);
    }

    Ok(response.body(Body::empty()).unwrap())
}

/// DELETE /storage/v1/object/:bucket/:key
///
/// Delete an object.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn delete_object(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let metadata_store = PgMetadataStore::new(state.pool.clone());
    let bucket = metadata_store
        .get_bucket_by_slug(org_id.into_uuid(), &bucket_ref)
        .await?
        .ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Check permission: storage:{bucket}:write or storage:*:write
    let write_perm = format!("storage:{}:write", bucket.slug);
    if !ctx.has_permission(&write_perm) && !ctx.has_permission("storage:*:write") {
        return Err(StorageError::PermissionDenied(format!(
            "{} permission required",
            write_perm
        )));
    }

    // Get object metadata
    let object = metadata_store
        .get_object_by_key(bucket.id, &key)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("{}/{}", bucket_ref, key)))?;

    // Delete from blob store
    let blob_store = get_blob_store(&state)?;
    let org_id_str = org_id.to_string();
    blob_store.delete(&org_id_str, &bucket.slug, &key).await?;

    // Delete metadata
    metadata_store.delete_object(object.id).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_header() {
        // Standard range
        let r = parse_range_header("bytes=0-99", 1000);
        assert_eq!(r, Some(0..100));

        // Open end range
        let r = parse_range_header("bytes=500-", 1000);
        assert_eq!(r, Some(500..1000));

        // Suffix range
        let r = parse_range_header("bytes=-100", 1000);
        assert_eq!(r, Some(900..1000));

        // Invalid format
        let r = parse_range_header("bytes=invalid", 1000);
        assert_eq!(r, None);

        // Out of bounds
        let r = parse_range_header("bytes=1000-2000", 1000);
        assert_eq!(r, None);
    }
}
