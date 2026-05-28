//! Bucket management routes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::StorageError;
use crate::middleware::StorageCtx;
use crate::state::StorageState;
use crate::store::{Bucket, BucketCreate, MetadataStore, PgMetadataStore};

/// Bucket creation request.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateBucketRequest {
    /// Bucket slug (URL-safe identifier).
    pub slug: String,
    /// Whether the bucket is publicly readable.
    #[serde(default)]
    pub is_public: bool,
}

impl CreateBucketRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        // Check length
        if self.slug.is_empty() || self.slug.len() > 63 {
            return Err("slug must be 1-63 characters".into());
        }

        // Check format: lowercase alphanumeric with hyphens, not starting/ending with hyphen
        if !crate::SLUG_REGEX.is_match(&self.slug) {
            return Err("slug must be lowercase alphanumeric with hyphens, not starting or ending with hyphen".into());
        }

        Ok(())
    }
}

/// Bucket update request.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateBucketRequest {
    /// New public status (optional).
    pub is_public: Option<bool>,
}

/// Bucket response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BucketResponse {
    /// Bucket ID.
    pub id: Uuid,
    /// Organization ID.
    pub org_id: Uuid,
    /// Bucket slug.
    pub slug: String,
    /// Whether publicly readable.
    pub is_public: bool,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

impl From<Bucket> for BucketResponse {
    fn from(b: Bucket) -> Self {
        Self {
            id: b.id,
            org_id: b.org_id,
            slug: b.slug,
            is_public: b.is_public,
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
        }
    }
}

/// Bucket list response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BucketsListResponse {
    /// List of buckets.
    pub buckets: Vec<BucketResponse>,
}

/// POST /storage/v1/buckets
///
/// Create a new bucket.
#[utoipa::path(
    post,
    path = "/storage/v1/buckets",
    tag = "storage.buckets",
    security(("bearer" = [])),
    request_body = CreateBucketRequest,
    responses(
        (status = 201, description = "Bucket created", body = BucketResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Permission denied"),
        (status = 409, description = "Bucket already exists"),
    )
)]
pub async fn create_bucket(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Json(req): Json<CreateBucketRequest>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    // Require org context
    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    // Check permission: storage:bucket:create
    if !ctx.has_permission("storage:bucket:create") {
        return Err(StorageError::PermissionDenied(
            "storage:bucket:create permission required".into(),
        ));
    }

    // Validate request
    req.validate()
        .map_err(StorageError::InvalidRequest)?;

    // Create the bucket
    let store = PgMetadataStore::new(state.pool.clone());
    let bucket = store
        .create_bucket(BucketCreate {
            org_id: org_id.into_uuid(),
            slug: req.slug,
            is_public: req.is_public,
            created_by: ctx.user_id().map(|u| u.into_uuid()),
        })
        .await?;

    Ok((StatusCode::CREATED, Json(BucketResponse::from(bucket))))
}

/// GET /storage/v1/buckets
///
/// List all buckets for the current organization.
#[utoipa::path(
    get,
    path = "/storage/v1/buckets",
    tag = "storage.buckets",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "List of buckets", body = BucketsListResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn list_buckets(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    // Require org context
    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    // List buckets
    let store = PgMetadataStore::new(state.pool.clone());
    let buckets = store.list_buckets(org_id.into_uuid()).await?;

    let response = BucketsListResponse {
        buckets: buckets.into_iter().map(BucketResponse::from).collect(),
    };

    Ok(Json(response))
}

/// GET /storage/v1/buckets/:ref
///
/// Get a single bucket by slug or ID.
#[utoipa::path(
    get,
    path = "/storage/v1/buckets/{bucket_ref}",
    tag = "storage.buckets",
    security(("bearer" = [])),
    params(
        ("bucket_ref" = String, Path, description = "Bucket slug or ID")
    ),
    responses(
        (status = 200, description = "Bucket details", body = BucketResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Bucket not found"),
    )
)]
pub async fn get_bucket(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path(bucket_ref): Path<String>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication (for now - public bucket access is at object level)
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    // Require org context
    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let store = PgMetadataStore::new(state.pool.clone());

    // Try to parse as UUID, otherwise treat as slug
    let bucket = if let Ok(id) = bucket_ref.parse::<Uuid>() {
        store.get_bucket(id).await?
    } else {
        store.get_bucket_by_slug(org_id.into_uuid(), &bucket_ref).await?
    };

    let bucket = bucket.ok_or_else(|| StorageError::BucketNotFound(bucket_ref))?;

    // Verify bucket belongs to org
    if bucket.org_id != org_id.into_uuid() {
        return Err(StorageError::BucketNotFound(bucket.slug));
    }

    Ok(Json(BucketResponse::from(bucket)))
}

/// PATCH /storage/v1/buckets/:ref
///
/// Update a bucket.
#[utoipa::path(
    patch,
    path = "/storage/v1/buckets/{bucket_ref}",
    tag = "storage.buckets",
    security(("bearer" = [])),
    params(
        ("bucket_ref" = String, Path, description = "Bucket slug or ID")
    ),
    request_body = UpdateBucketRequest,
    responses(
        (status = 200, description = "Bucket updated", body = BucketResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Bucket not found"),
    )
)]
pub async fn update_bucket(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path(bucket_ref): Path<String>,
    Json(req): Json<UpdateBucketRequest>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    // Require org context
    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let store = PgMetadataStore::new(state.pool.clone());

    // Get the bucket first
    let bucket = if let Ok(id) = bucket_ref.parse::<Uuid>() {
        store.get_bucket(id).await?
    } else {
        store.get_bucket_by_slug(org_id.into_uuid(), &bucket_ref).await?
    };

    let bucket = bucket.ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Verify bucket belongs to org
    if bucket.org_id != org_id.into_uuid() {
        return Err(StorageError::BucketNotFound(bucket_ref));
    }

    // Check permission: storage:{bucket_slug}:admin
    let admin_perm = format!("storage:{}:admin", bucket.slug);
    if !ctx.has_permission(&admin_perm) && !ctx.has_permission("storage:*:admin") {
        return Err(StorageError::PermissionDenied(format!(
            "{} permission required",
            admin_perm
        )));
    }

    // Update the bucket
    let updated = store.update_bucket(bucket.id, req.is_public).await?;

    Ok(Json(BucketResponse::from(updated)))
}

/// DELETE /storage/v1/buckets/:ref
///
/// Delete a bucket.
#[utoipa::path(
    delete,
    path = "/storage/v1/buckets/{bucket_ref}",
    tag = "storage.buckets",
    security(("bearer" = [])),
    params(
        ("bucket_ref" = String, Path, description = "Bucket slug or ID")
    ),
    responses(
        (status = 204, description = "Bucket deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Bucket not found"),
    )
)]
pub async fn delete_bucket(
    State(state): State<StorageState>,
    Extension(ctx): Extension<StorageCtx>,
    Path(bucket_ref): Path<String>,
) -> Result<impl IntoResponse, StorageError> {
    // Require authentication
    if ctx.is_anonymous {
        return Err(StorageError::AuthRequired);
    }

    // Require org context
    let org_id = ctx
        .org_id()
        .ok_or_else(|| StorageError::InvalidRequest("organization context required".into()))?;

    let store = PgMetadataStore::new(state.pool.clone());

    // Get the bucket first
    let bucket = if let Ok(id) = bucket_ref.parse::<Uuid>() {
        store.get_bucket(id).await?
    } else {
        store.get_bucket_by_slug(org_id.into_uuid(), &bucket_ref).await?
    };

    let bucket = bucket.ok_or_else(|| StorageError::BucketNotFound(bucket_ref.clone()))?;

    // Verify bucket belongs to org
    if bucket.org_id != org_id.into_uuid() {
        return Err(StorageError::BucketNotFound(bucket_ref));
    }

    // Check permission: storage:{bucket_slug}:admin
    let admin_perm = format!("storage:{}:admin", bucket.slug);
    if !ctx.has_permission(&admin_perm) && !ctx.has_permission("storage:*:admin") {
        return Err(StorageError::PermissionDenied(format!(
            "{} permission required",
            admin_perm
        )));
    }

    // Delete the bucket (cascades to objects due to FK)
    store.delete_bucket(bucket.id).await?;

    Ok(StatusCode::NO_CONTENT)
}
