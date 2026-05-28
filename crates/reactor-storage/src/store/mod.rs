//! Storage store abstractions.
//!
//! Provides traits and implementations for:
//! - MetadataStore: Bucket/object metadata in PostgreSQL
//! - BlobStore: Actual blob storage (FS or S3)

mod metadata;
pub mod blob;

pub use metadata::{
    Bucket, BucketCreate, MetadataStore, Object, ObjectCreate, ObjectUpdate,
    PgMetadataStore, StoredPolicy, MultipartUpload, MultipartPart,
};
pub use blob::{BlobMeta, BlobStore, ByteStream, SignedUrl};
