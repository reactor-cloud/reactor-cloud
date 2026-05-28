//! Metadata store for buckets and objects.

use crate::error::StorageError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Bucket record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Bucket {
    /// Unique bucket ID.
    pub id: Uuid,
    /// Organization that owns this bucket.
    pub org_id: Uuid,
    /// URL-safe slug (e.g., "avatars").
    pub slug: String,
    /// Whether the bucket allows anonymous read access.
    pub is_public: bool,
    /// When the bucket was created.
    pub created_at: DateTime<Utc>,
    /// When the bucket was last updated.
    pub updated_at: DateTime<Utc>,
    /// User who created the bucket.
    pub created_by: Option<Uuid>,
}

/// Input for creating a bucket.
#[derive(Debug, Clone)]
pub struct BucketCreate {
    /// Organization ID.
    pub org_id: Uuid,
    /// Bucket slug.
    pub slug: String,
    /// Whether public.
    pub is_public: bool,
    /// Creator user ID.
    pub created_by: Option<Uuid>,
}

/// Object record (metadata only).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Object {
    /// Unique object ID.
    pub id: Uuid,
    /// Bucket containing this object.
    pub bucket_id: Uuid,
    /// Object key (path within bucket).
    pub key: String,
    /// MIME content type.
    pub content_type: Option<String>,
    /// Size in bytes.
    pub content_length: i64,
    /// ETag for cache validation.
    pub etag: Option<String>,
    /// User-defined metadata.
    pub metadata: serde_json::Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Creator user ID.
    pub created_by: Option<Uuid>,
}

/// Input for creating an object.
#[derive(Debug, Clone)]
pub struct ObjectCreate {
    /// Bucket ID.
    pub bucket_id: Uuid,
    /// Object key.
    pub key: String,
    /// Content type.
    pub content_type: Option<String>,
    /// Size in bytes.
    pub content_length: i64,
    /// ETag.
    pub etag: Option<String>,
    /// User metadata.
    pub metadata: serde_json::Value,
    /// Creator user ID.
    pub created_by: Option<Uuid>,
}

/// Input for updating an object.
#[derive(Debug, Clone, Default)]
pub struct ObjectUpdate {
    /// New content type.
    pub content_type: Option<String>,
    /// New size.
    pub content_length: Option<i64>,
    /// New ETag.
    pub etag: Option<String>,
    /// Updated metadata (merged with existing).
    pub metadata: Option<serde_json::Value>,
}

/// Stored policy for a bucket.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StoredPolicy {
    /// Policy ID.
    pub id: i64,
    /// Bucket ID.
    pub bucket_id: Uuid,
    /// Policy name.
    pub name: String,
    /// Scopes (read, write).
    pub scopes: Vec<String>,
    /// USING clause AST.
    pub using_ast: Option<serde_json::Value>,
    /// CHECK clause AST.
    pub check_ast: Option<serde_json::Value>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Creator user ID.
    pub created_by: Option<Uuid>,
}

/// Multipart upload record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MultipartUpload {
    /// Upload ID.
    pub id: Uuid,
    /// Bucket ID.
    pub bucket_id: Uuid,
    /// Target object key.
    pub key: String,
    /// Content type.
    pub content_type: Option<String>,
    /// User metadata.
    pub metadata: serde_json::Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Creator user ID.
    pub created_by: Option<Uuid>,
    /// Expiration timestamp.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Multipart upload part.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MultipartPart {
    /// Upload ID.
    pub upload_id: Uuid,
    /// Part number.
    pub part_number: i32,
    /// Part ETag.
    pub etag: String,
    /// Part size.
    pub size: i64,
    /// Upload timestamp.
    pub uploaded_at: DateTime<Utc>,
}

/// Metadata store trait.
#[async_trait]
pub trait MetadataStore: Send + Sync {
    /// Run metadata schema migrations.
    async fn run_metadata_migrations(&self) -> Result<(), StorageError>;

    // Buckets
    /// Create a new bucket.
    async fn create_bucket(&self, input: BucketCreate) -> Result<Bucket, StorageError>;
    /// Get bucket by ID.
    async fn get_bucket(&self, id: Uuid) -> Result<Option<Bucket>, StorageError>;
    /// Get bucket by org and slug.
    async fn get_bucket_by_slug(&self, org_id: Uuid, slug: &str) -> Result<Option<Bucket>, StorageError>;
    /// List buckets for an organization.
    async fn list_buckets(&self, org_id: Uuid) -> Result<Vec<Bucket>, StorageError>;
    /// Update bucket.
    async fn update_bucket(&self, id: Uuid, is_public: Option<bool>) -> Result<Bucket, StorageError>;
    /// Delete bucket.
    async fn delete_bucket(&self, id: Uuid) -> Result<(), StorageError>;

    // Objects
    /// Create or update object metadata (upsert by bucket_id + key).
    async fn upsert_object(&self, input: ObjectCreate) -> Result<Object, StorageError>;
    /// Get object by ID.
    async fn get_object(&self, id: Uuid) -> Result<Option<Object>, StorageError>;
    /// Get object by bucket and key.
    async fn get_object_by_key(&self, bucket_id: Uuid, key: &str) -> Result<Option<Object>, StorageError>;
    /// List objects in a bucket with optional prefix.
    async fn list_objects(&self, bucket_id: Uuid, prefix: Option<&str>, limit: i64, offset: i64) -> Result<Vec<Object>, StorageError>;
    /// Delete object metadata.
    async fn delete_object(&self, id: Uuid) -> Result<(), StorageError>;

    // Policies
    /// List policies for a bucket.
    async fn list_policies(&self, bucket_id: Uuid) -> Result<Vec<StoredPolicy>, StorageError>;

    // Multipart uploads
    /// Create a multipart upload record.
    async fn create_multipart_upload(
        &self,
        bucket_id: Uuid,
        key: &str,
        upload_id: &str,
        created_by: Option<Uuid>,
        content_type: Option<&str>,
    ) -> Result<MultipartUpload, StorageError>;

    /// Get a multipart upload by upload ID.
    async fn get_multipart_upload(
        &self,
        bucket_id: Uuid,
        upload_id: &str,
    ) -> Result<Option<MultipartUpload>, StorageError>;

    /// Add a part to a multipart upload.
    async fn add_multipart_part(
        &self,
        bucket_id: Uuid,
        upload_id: &str,
        part_number: i32,
        etag: &str,
        size: i64,
    ) -> Result<MultipartPart, StorageError>;

    /// List parts for a multipart upload.
    async fn list_multipart_parts(
        &self,
        bucket_id: Uuid,
        upload_id: &str,
    ) -> Result<Vec<MultipartPart>, StorageError>;

    /// Delete a multipart upload and its parts.
    async fn delete_multipart_upload(
        &self,
        bucket_id: Uuid,
        upload_id: &str,
    ) -> Result<(), StorageError>;
}

/// PostgreSQL implementation of MetadataStore.
#[derive(Clone)]
pub struct PgMetadataStore {
    pool: PgPool,
}

impl PgMetadataStore {
    /// Create a new PgMetadataStore.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the underlying pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl MetadataStore for PgMetadataStore {
    async fn run_metadata_migrations(&self) -> Result<(), StorageError> {
        // Run migrations in order
        let migrations = [
            include_str!("../../migrations/001_metadata.sql"),
            include_str!("../../migrations/002_policies.sql"),
            include_str!("../../migrations/003_multipart.sql"),
            include_str!("../../migrations/004_audit.sql"),
        ];

        for migration in migrations {
            sqlx::raw_sql(migration)
                .execute(&self.pool)
                .await
                .map_err(|e| StorageError::Database(e))?;
        }

        Ok(())
    }

    // Buckets

    async fn create_bucket(&self, input: BucketCreate) -> Result<Bucket, StorageError> {
        let id = Uuid::now_v7();
        let bucket = sqlx::query_as::<_, Bucket>(
            r#"
            INSERT INTO _reactor_storage.buckets (id, org_id, slug, is_public, created_by)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(input.org_id)
        .bind(&input.slug)
        .bind(input.is_public)
        .bind(input.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
                StorageError::BucketExists(input.slug.clone())
            }
            _ => StorageError::Database(e),
        })?;

        Ok(bucket)
    }

    async fn get_bucket(&self, id: Uuid) -> Result<Option<Bucket>, StorageError> {
        let bucket = sqlx::query_as::<_, Bucket>(
            "SELECT * FROM _reactor_storage.buckets WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(bucket)
    }

    async fn get_bucket_by_slug(&self, org_id: Uuid, slug: &str) -> Result<Option<Bucket>, StorageError> {
        let bucket = sqlx::query_as::<_, Bucket>(
            "SELECT * FROM _reactor_storage.buckets WHERE org_id = $1 AND slug = $2",
        )
        .bind(org_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        Ok(bucket)
    }

    async fn list_buckets(&self, org_id: Uuid) -> Result<Vec<Bucket>, StorageError> {
        let buckets = sqlx::query_as::<_, Bucket>(
            "SELECT * FROM _reactor_storage.buckets WHERE org_id = $1 ORDER BY slug",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(buckets)
    }

    async fn update_bucket(&self, id: Uuid, is_public: Option<bool>) -> Result<Bucket, StorageError> {
        let bucket = if let Some(public) = is_public {
            sqlx::query_as::<_, Bucket>(
                r#"
                UPDATE _reactor_storage.buckets
                SET is_public = $2, updated_at = now()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(id)
            .bind(public)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Bucket>(
                "SELECT * FROM _reactor_storage.buckets WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
        };

        bucket.ok_or_else(|| StorageError::BucketNotFound(id.to_string()))
    }

    async fn delete_bucket(&self, id: Uuid) -> Result<(), StorageError> {
        let result = sqlx::query("DELETE FROM _reactor_storage.buckets WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::BucketNotFound(id.to_string()));
        }

        Ok(())
    }

    // Objects

    async fn upsert_object(&self, input: ObjectCreate) -> Result<Object, StorageError> {
        let id = Uuid::now_v7();
        let object = sqlx::query_as::<_, Object>(
            r#"
            INSERT INTO _reactor_storage.objects (id, bucket_id, key, content_type, content_length, etag, metadata, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (bucket_id, key) DO UPDATE SET
                content_type = EXCLUDED.content_type,
                content_length = EXCLUDED.content_length,
                etag = EXCLUDED.etag,
                metadata = EXCLUDED.metadata,
                updated_at = now()
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(input.bucket_id)
        .bind(&input.key)
        .bind(&input.content_type)
        .bind(input.content_length)
        .bind(&input.etag)
        .bind(&input.metadata)
        .bind(input.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(object)
    }

    async fn get_object(&self, id: Uuid) -> Result<Option<Object>, StorageError> {
        let object = sqlx::query_as::<_, Object>(
            "SELECT * FROM _reactor_storage.objects WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(object)
    }

    async fn get_object_by_key(&self, bucket_id: Uuid, key: &str) -> Result<Option<Object>, StorageError> {
        let object = sqlx::query_as::<_, Object>(
            "SELECT * FROM _reactor_storage.objects WHERE bucket_id = $1 AND key = $2",
        )
        .bind(bucket_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(object)
    }

    async fn list_objects(&self, bucket_id: Uuid, prefix: Option<&str>, limit: i64, offset: i64) -> Result<Vec<Object>, StorageError> {
        let objects = if let Some(prefix) = prefix {
            sqlx::query_as::<_, Object>(
                r#"
                SELECT * FROM _reactor_storage.objects
                WHERE bucket_id = $1 AND key LIKE $2
                ORDER BY key
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(bucket_id)
            .bind(format!("{}%", prefix))
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Object>(
                r#"
                SELECT * FROM _reactor_storage.objects
                WHERE bucket_id = $1
                ORDER BY key
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(bucket_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(objects)
    }

    async fn delete_object(&self, id: Uuid) -> Result<(), StorageError> {
        let result = sqlx::query("DELETE FROM _reactor_storage.objects WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }

        Ok(())
    }

    // Policies

    async fn list_policies(&self, bucket_id: Uuid) -> Result<Vec<StoredPolicy>, StorageError> {
        let policies = sqlx::query_as::<_, StoredPolicy>(
            "SELECT * FROM _reactor_storage.policies WHERE bucket_id = $1 ORDER BY name",
        )
        .bind(bucket_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(policies)
    }

    // Multipart uploads

    async fn create_multipart_upload(
        &self,
        bucket_id: Uuid,
        key: &str,
        upload_id: &str,
        created_by: Option<Uuid>,
        content_type: Option<&str>,
    ) -> Result<MultipartUpload, StorageError> {
        let id = Uuid::parse_str(upload_id)
            .map_err(|e| StorageError::InvalidRequest(format!("Invalid upload ID: {}", e)))?;

        let upload = sqlx::query_as::<_, MultipartUpload>(
            r#"
            INSERT INTO _reactor_storage.multipart_uploads
                (id, bucket_id, key, content_type, created_by)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(bucket_id)
        .bind(key)
        .bind(content_type)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(upload)
    }

    async fn get_multipart_upload(
        &self,
        bucket_id: Uuid,
        upload_id: &str,
    ) -> Result<Option<MultipartUpload>, StorageError> {
        let id = Uuid::parse_str(upload_id)
            .map_err(|e| StorageError::InvalidRequest(format!("Invalid upload ID: {}", e)))?;

        let upload = sqlx::query_as::<_, MultipartUpload>(
            "SELECT * FROM _reactor_storage.multipart_uploads WHERE bucket_id = $1 AND id = $2",
        )
        .bind(bucket_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(upload)
    }

    async fn add_multipart_part(
        &self,
        bucket_id: Uuid,
        upload_id: &str,
        part_number: i32,
        etag: &str,
        size: i64,
    ) -> Result<MultipartPart, StorageError> {
        let id = Uuid::parse_str(upload_id)
            .map_err(|e| StorageError::InvalidRequest(format!("Invalid upload ID: {}", e)))?;

        // Verify the upload exists
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM _reactor_storage.multipart_uploads WHERE bucket_id = $1 AND id = $2)",
        )
        .bind(bucket_id)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        if !exists {
            return Err(StorageError::NotFound(format!(
                "Upload {} not found",
                upload_id
            )));
        }

        let part = sqlx::query_as::<_, MultipartPart>(
            r#"
            INSERT INTO _reactor_storage.multipart_parts (upload_id, part_number, etag, size)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (upload_id, part_number) DO UPDATE SET
                etag = EXCLUDED.etag,
                size = EXCLUDED.size,
                uploaded_at = now()
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(part_number)
        .bind(etag)
        .bind(size)
        .fetch_one(&self.pool)
        .await?;

        Ok(part)
    }

    async fn list_multipart_parts(
        &self,
        bucket_id: Uuid,
        upload_id: &str,
    ) -> Result<Vec<MultipartPart>, StorageError> {
        let id = Uuid::parse_str(upload_id)
            .map_err(|e| StorageError::InvalidRequest(format!("Invalid upload ID: {}", e)))?;

        // Verify the upload belongs to this bucket
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM _reactor_storage.multipart_uploads WHERE bucket_id = $1 AND id = $2)",
        )
        .bind(bucket_id)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        if !exists {
            return Err(StorageError::NotFound(format!(
                "Upload {} not found",
                upload_id
            )));
        }

        let parts = sqlx::query_as::<_, MultipartPart>(
            "SELECT * FROM _reactor_storage.multipart_parts WHERE upload_id = $1 ORDER BY part_number",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        Ok(parts)
    }

    async fn delete_multipart_upload(
        &self,
        bucket_id: Uuid,
        upload_id: &str,
    ) -> Result<(), StorageError> {
        let id = Uuid::parse_str(upload_id)
            .map_err(|e| StorageError::InvalidRequest(format!("Invalid upload ID: {}", e)))?;

        // Delete parts first (foreign key constraint)
        sqlx::query("DELETE FROM _reactor_storage.multipart_parts WHERE upload_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        // Delete the upload
        let result = sqlx::query(
            "DELETE FROM _reactor_storage.multipart_uploads WHERE bucket_id = $1 AND id = $2",
        )
        .bind(bucket_id)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!(
                "Upload {} not found",
                upload_id
            )));
        }

        Ok(())
    }
}
