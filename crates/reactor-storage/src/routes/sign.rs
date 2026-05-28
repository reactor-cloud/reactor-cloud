//! Signed URL generation routes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
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

/// Request to generate a signed URL.
#[derive(Debug, Deserialize)]
pub struct SignUrlRequest {
    /// Duration in seconds for the URL to remain valid (default: 3600).
    #[serde(default = "default_expires_in")]
    pub expires_in: u64,
    /// Whether this is for upload (PUT) or download (GET). Default: false (download).
    #[serde(default)]
    pub for_upload: bool,
}

fn default_expires_in() -> u64 {
    3600 // 1 hour
}

/// Response with signed URL.
#[derive(Debug, Serialize)]
pub struct SignUrlResponse {
    /// The signed URL.
    pub url: String,
    /// Unix timestamp when the URL expires.
    pub expires_at: u64,
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
    Err(StorageError::Internal("No blob store configured".into()))
}

/// POST /storage/v1/sign/:bucket/:key
///
/// Generate a signed URL for an object.
#[cfg(any(feature = "fs", feature = "s3"))]
pub async fn create_signed_url(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path((bucket_ref, key)): Path<(String, String)>,
    Json(request): Json<SignUrlRequest>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication for generating signed URLs
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

    // Check permission based on whether it's for upload or download
    let required_perm = if request.for_upload {
        format!("storage:{}:write", bucket.slug)
    } else {
        format!("storage:{}:read", bucket.slug)
    };

    let wildcard_perm = if request.for_upload {
        "storage:*:write"
    } else {
        "storage:*:read"
    };

    // For public buckets, allow read without permission check
    if !bucket.is_public || request.for_upload {
        if !ctx.has_permission(&required_perm) && !ctx.has_permission(wildcard_perm) {
            return Err(StorageError::PermissionDenied(format!(
                "{} permission required",
                required_perm
            )));
        }
    }

    // Validate expires_in
    let max_expires = 7 * 24 * 3600; // 7 days max
    if request.expires_in > max_expires {
        return Err(StorageError::InvalidRequest(format!(
            "expires_in cannot exceed {} seconds (7 days)",
            max_expires
        )));
    }

    let expires_in = if request.expires_in == 0 {
        default_expires_in()
    } else {
        request.expires_in
    };

    // Generate signed URL
    let blob_store = get_blob_store(&state)?;
    let org_id_str = org_id.to_string();
    let signed = blob_store
        .sign_url(&org_id_str, &bucket.slug, &key, expires_in, request.for_upload)
        .await?;

    Ok((
        StatusCode::OK,
        Json(SignUrlResponse {
            url: signed.url,
            expires_at: signed.expires_at,
        }),
    ))
}
