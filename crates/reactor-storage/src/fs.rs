//! Filesystem storage backend.
//!
//! Provides local filesystem-based blob storage with atomic operations
//! and HMAC-based signed URLs.

use crate::error::StorageError;
use crate::store::{BlobMeta, BlobStore, ByteStream, SignedUrl};
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream;
use sha2::{Digest, Sha256};
use std::ops::Range;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

/// Filesystem-based blob store.
///
/// Stores objects in the local filesystem with the following layout:
/// `{base_path}/{org_id}/{bucket}/{key}`
///
/// Uses temp files + atomic rename for safe writes.
#[derive(Debug, Clone)]
pub struct FsBlobStore {
    /// Base path for storage.
    base_path: PathBuf,
    /// Secret for HMAC signing.
    signing_secret: Option<String>,
}

impl FsBlobStore {
    /// Create a new filesystem blob store.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            signing_secret: None,
        }
    }

    /// Set the signing secret for HMAC URLs.
    pub fn with_signing_secret(mut self, secret: impl Into<String>) -> Self {
        self.signing_secret = Some(secret.into());
        self
    }

    /// Get the full path for an object.
    fn object_path(&self, org_id: &str, bucket: &str, key: &str) -> PathBuf {
        self.base_path.join(org_id).join(bucket).join(key)
    }

    /// Get the temp path for atomic writes.
    fn temp_path(&self, org_id: &str, bucket: &str, key: &str) -> PathBuf {
        let filename = format!(
            ".tmp.{}.{}",
            uuid::Uuid::now_v7(),
            key.replace('/', "_")
        );
        self.base_path.join(org_id).join(bucket).join(filename)
    }

    /// Get the path for multipart upload parts directory.
    fn multipart_dir(&self, org_id: &str, bucket: &str, upload_id: &str) -> PathBuf {
        self.base_path
            .join(org_id)
            .join(bucket)
            .join(".multipart")
            .join(upload_id)
    }

    /// Get the path for a specific part file.
    fn part_path(&self, org_id: &str, bucket: &str, upload_id: &str, part_number: i32) -> PathBuf {
        self.multipart_dir(org_id, bucket, upload_id)
            .join(format!("part_{:05}", part_number))
    }

    /// Compute ETag (SHA256 hex) for data.
    fn compute_etag(data: &[u8]) -> String {
        let hash = Sha256::digest(data);
        format!("\"{}\"", hex::encode(hash))
    }

    /// Create HMAC signature for a URL.
    fn sign_hmac(&self, method: &str, path: &str, expires: u64) -> Result<String, StorageError> {
        use hmac::{Hmac, Mac};

        let secret = self
            .signing_secret
            .as_ref()
            .ok_or_else(|| StorageError::Internal("signing secret not configured".into()))?;

        let message = format!("{}:{}:{}", method, path, expires);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .map_err(|e| StorageError::Internal(format!("HMAC error: {}", e)))?;
        mac.update(message.as_bytes());
        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }
}

#[async_trait]
impl BlobStore for FsBlobStore {
    async fn put(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        data: Bytes,
        _content_type: Option<&str>,
    ) -> Result<String, StorageError> {
        let final_path = self.object_path(org_id, bucket, key);
        let temp_path = self.temp_path(org_id, bucket, key);

        // Ensure parent directory exists
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Compute ETag
        let etag = Self::compute_etag(&data);

        // Write to temp file
        fs::write(&temp_path, &data).await?;

        // Atomic rename
        fs::rename(&temp_path, &final_path).await.map_err(|e| {
            // Try to clean up temp file on error
            let _ = std::fs::remove_file(&temp_path);
            e
        })?;

        Ok(etag)
    }

    async fn get(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        range: Option<Range<u64>>,
    ) -> Result<(ByteStream, BlobMeta), StorageError> {
        let path = self.object_path(org_id, bucket, key);

        // Open the file
        let mut file = fs::File::open(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(format!("{}/{}/{}", org_id, bucket, key))
            } else {
                StorageError::Io(e)
            }
        })?;

        // Get file metadata
        let metadata = file.metadata().await?;
        let total_size = metadata.len();

        // Handle range request
        let (start, end) = if let Some(ref r) = range {
            (r.start, r.end.min(total_size))
        } else {
            (0, total_size)
        };

        // Seek to start position
        if start > 0 {
            file.seek(SeekFrom::Start(start)).await?;
        }

        // Read the data
        let read_size = (end - start) as usize;
        let mut buffer = vec![0u8; read_size];
        file.read_exact(&mut buffer).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                StorageError::InvalidRequest("range extends beyond file size".into())
            } else {
                StorageError::Io(e)
            }
        })?;

        let bytes = Bytes::from(buffer);
        let etag = Self::compute_etag(&bytes);

        let meta = BlobMeta {
            content_length: total_size,
            content_type: None, // FS doesn't store content type
            etag: Some(etag),
        };

        // Create a stream from the bytes
        let stream = stream::once(async move { Ok(bytes) });

        Ok((Box::pin(stream), meta))
    }

    async fn head(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<BlobMeta, StorageError> {
        let path = self.object_path(org_id, bucket, key);

        let metadata = fs::metadata(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(format!("{}/{}/{}", org_id, bucket, key))
            } else {
                StorageError::Io(e)
            }
        })?;

        Ok(BlobMeta {
            content_length: metadata.len(),
            content_type: None,
            etag: None, // Would need to read file to compute
        })
    }

    async fn delete(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<(), StorageError> {
        let path = self.object_path(org_id, bucket, key);

        fs::remove_file(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(format!("{}/{}/{}", org_id, bucket, key))
            } else {
                StorageError::Io(e)
            }
        })?;

        Ok(())
    }

    async fn exists(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<bool, StorageError> {
        let path = self.object_path(org_id, bucket, key);
        Ok(path.exists())
    }

    async fn sign_url(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        expires_in_secs: u64,
        for_upload: bool,
    ) -> Result<SignedUrl, StorageError> {
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + expires_in_secs;

        let method = if for_upload { "PUT" } else { "GET" };
        let path = format!("{}/{}/{}", org_id, bucket, key);
        let signature = self.sign_hmac(method, &path, expires_at)?;

        // The URL format is:
        // /storage/v1/object/{bucket}/{key}?signature={sig}&expires={exp}
        let url = format!(
            "/storage/v1/object/{}/{}?signature={}&expires={}",
            bucket, key, signature, expires_at
        );

        Ok(SignedUrl { url, expires_at })
    }

    // Multipart upload methods

    async fn create_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        _key: &str,
        _content_type: Option<&str>,
    ) -> Result<String, StorageError> {
        // Generate a unique upload ID
        let upload_id = uuid::Uuid::now_v7().to_string();

        // Create the multipart directory
        let multipart_dir = self.multipart_dir(org_id, bucket, &upload_id);
        fs::create_dir_all(&multipart_dir).await?;

        Ok(upload_id)
    }

    async fn upload_part(
        &self,
        org_id: &str,
        bucket: &str,
        _key: &str,
        upload_id: &str,
        part_number: i32,
        data: Bytes,
    ) -> Result<String, StorageError> {
        // Validate part number (S3 allows 1-10000)
        if !(1..=10000).contains(&part_number) {
            return Err(StorageError::InvalidRequest(format!(
                "Part number must be between 1 and 10000, got {}",
                part_number
            )));
        }

        // Ensure multipart directory exists
        let multipart_dir = self.multipart_dir(org_id, bucket, upload_id);
        if !multipart_dir.exists() {
            return Err(StorageError::NotFound(format!(
                "Upload {} not found",
                upload_id
            )));
        }

        // Write the part
        let part_path = self.part_path(org_id, bucket, upload_id, part_number);
        let etag = Self::compute_etag(&data);
        fs::write(&part_path, &data).await?;

        Ok(etag)
    }

    async fn complete_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: &[(i32, String)],
    ) -> Result<String, StorageError> {
        let multipart_dir = self.multipart_dir(org_id, bucket, upload_id);
        if !multipart_dir.exists() {
            return Err(StorageError::NotFound(format!(
                "Upload {} not found",
                upload_id
            )));
        }

        // Verify all parts exist and read them in order
        let mut all_data = Vec::new();
        for (part_number, _expected_etag) in parts {
            let part_path = self.part_path(org_id, bucket, upload_id, *part_number);
            if !part_path.exists() {
                return Err(StorageError::NotFound(format!(
                    "Part {} not found for upload {}",
                    part_number, upload_id
                )));
            }
            let part_data = fs::read(&part_path).await?;
            all_data.extend(part_data);
        }

        // Write the final object atomically
        let final_path = self.object_path(org_id, bucket, key);
        let temp_path = self.temp_path(org_id, bucket, key);

        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let etag = Self::compute_etag(&all_data);
        fs::write(&temp_path, &all_data).await?;
        fs::rename(&temp_path, &final_path).await?;

        // Clean up multipart directory
        fs::remove_dir_all(&multipart_dir).await?;

        Ok(etag)
    }

    async fn abort_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        _key: &str,
        upload_id: &str,
    ) -> Result<(), StorageError> {
        let multipart_dir = self.multipart_dir(org_id, bucket, upload_id);
        if multipart_dir.exists() {
            fs::remove_dir_all(&multipart_dir).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_put_get_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = FsBlobStore::new(temp_dir.path());

        let data = Bytes::from("hello world");
        let etag = store
            .put("org1", "bucket1", "test.txt", data.clone(), None)
            .await
            .unwrap();

        assert!(!etag.is_empty());

        // Get the object
        let (stream, meta) = store.get("org1", "bucket1", "test.txt", None).await.unwrap();
        assert_eq!(meta.content_length, 11);

        // Collect the stream
        use futures::StreamExt;
        let chunks: Vec<Bytes> = stream.map(|r| r.unwrap()).collect().await;
        let result: Vec<u8> = chunks.into_iter().flat_map(|b| b.to_vec()).collect();
        assert_eq!(result, data.to_vec());

        // Delete
        store.delete("org1", "bucket1", "test.txt").await.unwrap();

        // Verify deleted
        assert!(!store.exists("org1", "bucket1", "test.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_range_get() {
        let temp_dir = TempDir::new().unwrap();
        let store = FsBlobStore::new(temp_dir.path());

        let data = Bytes::from("hello world");
        store
            .put("org1", "bucket1", "test.txt", data, None)
            .await
            .unwrap();

        // Get a range
        let (stream, _meta) = store
            .get("org1", "bucket1", "test.txt", Some(0..5))
            .await
            .unwrap();

        use futures::StreamExt;
        let chunks: Vec<Bytes> = stream.map(|r| r.unwrap()).collect().await;
        let result: Vec<u8> = chunks.into_iter().flat_map(|b| b.to_vec()).collect();
        assert_eq!(result, b"hello".to_vec());
    }

    #[tokio::test]
    async fn test_signed_url() {
        let temp_dir = TempDir::new().unwrap();
        let store = FsBlobStore::new(temp_dir.path()).with_signing_secret("test-secret");

        let signed = store
            .sign_url("org1", "bucket1", "test.txt", 3600, false)
            .await
            .unwrap();

        assert!(signed.url.contains("signature="));
        assert!(signed.url.contains("expires="));
    }

    #[tokio::test]
    async fn test_multipart_upload() {
        let temp_dir = TempDir::new().unwrap();
        let store = FsBlobStore::new(temp_dir.path());

        // Create multipart upload
        let upload_id = store
            .create_multipart("org1", "bucket1", "big-file.bin", Some("application/octet-stream"))
            .await
            .unwrap();

        assert!(!upload_id.is_empty());

        // Upload parts
        let part1_data = Bytes::from(vec![0u8; 1024]); // 1KB
        let part2_data = Bytes::from(vec![1u8; 1024]); // 1KB

        let etag1 = store
            .upload_part("org1", "bucket1", "big-file.bin", &upload_id, 1, part1_data.clone())
            .await
            .unwrap();

        let etag2 = store
            .upload_part("org1", "bucket1", "big-file.bin", &upload_id, 2, part2_data.clone())
            .await
            .unwrap();

        assert!(!etag1.is_empty());
        assert!(!etag2.is_empty());

        // Complete upload
        let parts = vec![(1, etag1), (2, etag2)];
        let final_etag = store
            .complete_multipart("org1", "bucket1", "big-file.bin", &upload_id, &parts)
            .await
            .unwrap();

        assert!(!final_etag.is_empty());

        // Verify the object exists and has correct size
        let (stream, meta) = store
            .get("org1", "bucket1", "big-file.bin", None)
            .await
            .unwrap();

        assert_eq!(meta.content_length, 2048); // 2KB total

        use futures::StreamExt;
        let chunks: Vec<Bytes> = stream.map(|r| r.unwrap()).collect().await;
        let result: Vec<u8> = chunks.into_iter().flat_map(|b| b.to_vec()).collect();

        // First half should be zeros, second half should be ones
        assert!(result[0..1024].iter().all(|&b| b == 0));
        assert!(result[1024..2048].iter().all(|&b| b == 1));
    }

    #[tokio::test]
    async fn test_multipart_abort() {
        let temp_dir = TempDir::new().unwrap();
        let store = FsBlobStore::new(temp_dir.path());

        // Create multipart upload
        let upload_id = store
            .create_multipart("org1", "bucket1", "aborted.bin", None)
            .await
            .unwrap();

        // Upload a part
        let part_data = Bytes::from(vec![0u8; 512]);
        store
            .upload_part("org1", "bucket1", "aborted.bin", &upload_id, 1, part_data)
            .await
            .unwrap();

        // Abort
        store
            .abort_multipart("org1", "bucket1", "aborted.bin", &upload_id)
            .await
            .unwrap();

        // Verify the object doesn't exist
        assert!(!store.exists("org1", "bucket1", "aborted.bin").await.unwrap());
    }
}
