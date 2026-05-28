//! HTTP route handlers.

pub mod buckets;
pub mod health;
mod metrics;
#[cfg(any(feature = "fs", feature = "s3"))]
mod multipart;
#[cfg(any(feature = "fs", feature = "s3"))]
mod objects;
#[cfg(any(feature = "fs", feature = "s3"))]
mod sign;

pub use buckets::{create_bucket, delete_bucket, get_bucket, list_buckets, update_bucket};
pub use health::health;
pub use metrics::metrics;

#[cfg(any(feature = "fs", feature = "s3"))]
pub use multipart::{
    abort_multipart_upload, complete_multipart_upload, create_multipart_upload, upload_part,
};
#[cfg(any(feature = "fs", feature = "s3"))]
pub use objects::{delete_object, get_object, head_object, put_object};
#[cfg(any(feature = "fs", feature = "s3"))]
pub use sign::create_signed_url;
