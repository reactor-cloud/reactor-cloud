//! Error types for the deploy bundle crate.

use thiserror::Error;

/// Errors that can occur during bundle operations.
#[derive(Debug, Error)]
pub enum BundleError {
    /// IO error during pack/unpack.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Manifest is missing from the bundle.
    #[error("manifest.json not found in bundle")]
    MissingManifest,

    /// Invalid manifest structure.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    /// Content hash mismatch.
    #[error("hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        /// Path of the file with mismatched hash.
        path: String,
        /// Expected SHA-256 hash.
        expected: String,
        /// Actual SHA-256 hash.
        actual: String,
    },

    /// Referenced file not found in bundle.
    #[error("file not found in bundle: {0}")]
    FileNotFound(String),

    /// Version incompatibility.
    #[error("version incompatibility: bundle requires {required}, server is {actual}")]
    VersionMismatch {
        /// Version required by the bundle.
        required: String,
        /// Actual server version.
        actual: String,
    },

    /// Bundle is too large.
    #[error("bundle too large: {size} bytes exceeds limit of {limit} bytes")]
    TooLarge {
        /// Actual size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        limit: u64,
    },

    /// Compression error.
    #[error("compression error: {0}")]
    Compression(String),

    /// Decompression error.
    #[error("decompression error: {0}")]
    Decompression(String),
}
