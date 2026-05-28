//! Blob store trait for storage backends.

use crate::error::StorageError;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::ops::Range;
use std::pin::Pin;

/// Stream of bytes for reading objects.
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, StorageError>> + Send>>;

/// Object metadata from blob store.
#[derive(Debug, Clone)]
pub struct BlobMeta {
    /// Content length in bytes.
    pub content_length: u64,
    /// Content type (MIME).
    pub content_type: Option<String>,
    /// ETag for cache validation.
    pub etag: Option<String>,
}

/// Signed URL information.
#[derive(Debug, Clone)]
pub struct SignedUrl {
    /// The signed URL.
    pub url: String,
    /// Expiration timestamp (Unix seconds).
    pub expires_at: u64,
}

/// Blob store trait for storage backends.
///
/// Implementations handle the actual blob storage (filesystem, S3, etc.).
/// Metadata is handled separately by MetadataStore.
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Put an object into the store.
    ///
    /// # Arguments
    /// * `org_id` - Organization ID (for key prefixing)
    /// * `bucket` - Bucket slug
    /// * `key` - Object key
    /// * `data` - Object data
    /// * `content_type` - Optional MIME type
    ///
    /// # Returns
    /// ETag of the stored object.
    async fn put(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<String, StorageError>;

    /// Get an object from the store.
    ///
    /// # Arguments
    /// * `org_id` - Organization ID
    /// * `bucket` - Bucket slug
    /// * `key` - Object key
    /// * `range` - Optional byte range for partial reads
    ///
    /// # Returns
    /// Tuple of (stream, metadata).
    async fn get(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        range: Option<Range<u64>>,
    ) -> Result<(ByteStream, BlobMeta), StorageError>;

    /// Get object metadata without downloading content.
    async fn head(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<BlobMeta, StorageError>;

    /// Delete an object from the store.
    async fn delete(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<(), StorageError>;

    /// Check if an object exists.
    async fn exists(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<bool, StorageError>;

    /// Generate a signed URL for the object.
    ///
    /// # Arguments
    /// * `org_id` - Organization ID
    /// * `bucket` - Bucket slug
    /// * `key` - Object key
    /// * `expires_in_secs` - URL validity duration
    /// * `for_upload` - If true, generate upload URL; otherwise download URL
    async fn sign_url(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        expires_in_secs: u64,
        for_upload: bool,
    ) -> Result<SignedUrl, StorageError>;

    // Multipart upload methods (implemented in PR 11)

    /// Create a multipart upload.
    async fn create_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        content_type: Option<&str>,
    ) -> Result<String, StorageError> {
        // Default implementation returns not supported
        let _ = (org_id, bucket, key, content_type);
        Err(StorageError::Internal("Multipart uploads not supported by this backend".into()))
    }

    /// Upload a part for a multipart upload.
    async fn upload_part(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: i32,
        data: Bytes,
    ) -> Result<String, StorageError> {
        // Default implementation returns not supported
        let _ = (org_id, bucket, key, upload_id, part_number, data);
        Err(StorageError::Internal("Multipart uploads not supported by this backend".into()))
    }

    /// Complete a multipart upload.
    async fn complete_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: &[(i32, String)], // (part_number, etag)
    ) -> Result<String, StorageError> {
        // Default implementation returns not supported
        let _ = (org_id, bucket, key, upload_id, parts);
        Err(StorageError::Internal("Multipart uploads not supported by this backend".into()))
    }

    /// Abort a multipart upload.
    async fn abort_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<(), StorageError> {
        // Default implementation returns not supported
        let _ = (org_id, bucket, key, upload_id);
        Err(StorageError::Internal("Multipart uploads not supported by this backend".into()))
    }
}
