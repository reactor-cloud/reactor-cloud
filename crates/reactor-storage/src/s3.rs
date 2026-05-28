//! S3 storage backend.
//!
//! Provides S3-compatible storage using AWS SDK with native presigned URLs.
//!
//! Uses a single physical S3 bucket with key-prefix layout:
//! `{org_id}/{bucket}/{key}`

use crate::error::StorageError;
use crate::store::{BlobMeta, BlobStore, ByteStream, SignedUrl};
use async_trait::async_trait;
use aws_config::Region;
use aws_sdk_s3::{
    config::Builder as S3ConfigBuilder,
    presigning::PresigningConfig,
    primitives::ByteStream as S3ByteStream,
    Client,
};
use bytes::Bytes;
use futures::stream;
use sha2::{Digest, Sha256};
use std::ops::Range;
use std::time::Duration;

/// S3-based blob store.
///
/// Stores objects in a single S3 bucket with the following key layout:
/// `{org_id}/{bucket}/{key}`
#[derive(Debug, Clone)]
pub struct S3BlobStore {
    client: Client,
    bucket: String,
}

impl S3BlobStore {
    /// Create a new S3 blob store.
    pub fn new(client: Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    /// Create from configuration.
    pub async fn from_config(
        bucket: impl Into<String>,
        region: Option<String>,
        endpoint: Option<String>,
    ) -> Result<Self, StorageError> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

        let mut s3_config = S3ConfigBuilder::from(&config);

        if let Some(r) = region {
            s3_config = s3_config.region(Region::new(r));
        }

        if let Some(e) = endpoint {
            s3_config = s3_config
                .endpoint_url(e)
                .force_path_style(true); // Required for MinIO/localstack
        }

        let client = Client::from_conf(s3_config.build());

        Ok(Self::new(client, bucket))
    }

    /// Get the full S3 key for an object.
    fn s3_key(&self, org_id: &str, bucket: &str, key: &str) -> String {
        format!("{}/{}/{}", org_id, bucket, key)
    }

    /// Compute ETag (SHA256 hex) for data.
    fn compute_etag(data: &[u8]) -> String {
        let hash = Sha256::digest(data);
        format!("\"{}\"", hex::encode(hash))
    }
}

#[async_trait]
impl BlobStore for S3BlobStore {
    async fn put(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<String, StorageError> {
        let s3_key = self.s3_key(org_id, bucket, key);
        let etag = Self::compute_etag(&data);

        let mut request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&s3_key)
            .body(S3ByteStream::from(data.to_vec()));

        if let Some(ct) = content_type {
            request = request.content_type(ct);
        }

        request.send().await.map_err(|e| {
            StorageError::Internal(format!("S3 put error: {}", e))
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
        let s3_key = self.s3_key(org_id, bucket, key);

        let mut request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&s3_key);

        if let Some(ref r) = range {
            request = request.range(format!("bytes={}-{}", r.start, r.end - 1));
        }

        let response = request.send().await.map_err(|e| {
            let err = e.into_service_error();
            if err.is_no_such_key() {
                StorageError::NotFound(format!("{}/{}/{}", org_id, bucket, key))
            } else {
                StorageError::Internal(format!("S3 get error: {:?}", err))
            }
        })?;

        let meta = BlobMeta {
            content_length: response.content_length().unwrap_or(0) as u64,
            content_type: response.content_type().map(String::from),
            etag: response.e_tag().map(String::from),
        };

        // Collect the S3 body and return as a single-chunk stream
        // For large files, consider implementing proper streaming
        let body_bytes = response.body.collect().await.map_err(|e| {
            StorageError::Internal(format!("S3 body collect error: {}", e))
        })?;

        let data = body_bytes.into_bytes();
        let byte_stream = stream::once(async move { Ok::<_, StorageError>(data) });

        Ok((Box::pin(byte_stream), meta))
    }

    async fn head(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<BlobMeta, StorageError> {
        let s3_key = self.s3_key(org_id, bucket, key);

        let response = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&s3_key)
            .send()
            .await
            .map_err(|e| {
                let err = e.into_service_error();
                if err.is_not_found() {
                    StorageError::NotFound(format!("{}/{}/{}", org_id, bucket, key))
                } else {
                    StorageError::Internal(format!("S3 head error: {:?}", err))
                }
            })?;

        Ok(BlobMeta {
            content_length: response.content_length().unwrap_or(0) as u64,
            content_type: response.content_type().map(String::from),
            etag: response.e_tag().map(String::from),
        })
    }

    async fn delete(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<(), StorageError> {
        let s3_key = self.s3_key(org_id, bucket, key);

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&s3_key)
            .send()
            .await
            .map_err(|e| {
                StorageError::Internal(format!("S3 delete error: {}", e))
            })?;

        Ok(())
    }

    async fn exists(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<bool, StorageError> {
        match self.head(org_id, bucket, key).await {
            Ok(_) => Ok(true),
            Err(StorageError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn sign_url(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        expires_in_secs: u64,
        for_upload: bool,
    ) -> Result<SignedUrl, StorageError> {
        let s3_key = self.s3_key(org_id, bucket, key);
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + expires_in_secs;

        let presign_config = PresigningConfig::expires_in(Duration::from_secs(expires_in_secs))
            .map_err(|e| StorageError::Internal(format!("Presign config error: {}", e)))?;

        let url = if for_upload {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&s3_key)
                .presigned(presign_config)
                .await
                .map_err(|e| StorageError::Internal(format!("Presign error: {}", e)))?
                .uri()
                .to_string()
        } else {
            self.client
                .get_object()
                .bucket(&self.bucket)
                .key(&s3_key)
                .presigned(presign_config)
                .await
                .map_err(|e| StorageError::Internal(format!("Presign error: {}", e)))?
                .uri()
                .to_string()
        };

        Ok(SignedUrl { url, expires_at })
    }

    // Multipart upload methods

    async fn create_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        content_type: Option<&str>,
    ) -> Result<String, StorageError> {
        let s3_key = self.s3_key(org_id, bucket, key);

        let mut request = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(&s3_key);

        if let Some(ct) = content_type {
            request = request.content_type(ct);
        }

        let response = request.send().await.map_err(|e| {
            StorageError::Internal(format!("S3 create multipart error: {}", e))
        })?;

        response
            .upload_id()
            .map(String::from)
            .ok_or_else(|| StorageError::Internal("No upload ID returned".into()))
    }

    async fn upload_part(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: i32,
        data: Bytes,
    ) -> Result<String, StorageError> {
        let s3_key = self.s3_key(org_id, bucket, key);

        let response = self
            .client
            .upload_part()
            .bucket(&self.bucket)
            .key(&s3_key)
            .upload_id(upload_id)
            .part_number(part_number)
            .body(S3ByteStream::from(data.to_vec()))
            .send()
            .await
            .map_err(|e| {
                StorageError::Internal(format!("S3 upload part error: {}", e))
            })?;

        response
            .e_tag()
            .map(String::from)
            .ok_or_else(|| StorageError::Internal("No ETag returned for part".into()))
    }

    async fn complete_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: &[(i32, String)],
    ) -> Result<String, StorageError> {
        use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};

        let s3_key = self.s3_key(org_id, bucket, key);

        let completed_parts: Vec<CompletedPart> = parts
            .iter()
            .map(|(part_num, etag)| {
                CompletedPart::builder()
                    .part_number(*part_num)
                    .e_tag(etag)
                    .build()
            })
            .collect();

        let completed_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(completed_parts))
            .build();

        let response = self
            .client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(&s3_key)
            .upload_id(upload_id)
            .multipart_upload(completed_upload)
            .send()
            .await
            .map_err(|e| {
                StorageError::Internal(format!("S3 complete multipart error: {}", e))
            })?;

        response
            .e_tag()
            .map(String::from)
            .ok_or_else(|| StorageError::Internal("No ETag returned for completed upload".into()))
    }

    async fn abort_multipart(
        &self,
        org_id: &str,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<(), StorageError> {
        let s3_key = self.s3_key(org_id, bucket, key);

        self.client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(&s3_key)
            .upload_id(upload_id)
            .send()
            .await
            .map_err(|e| {
                StorageError::Internal(format!("S3 abort multipart error: {}", e))
            })?;

        Ok(())
    }
}
