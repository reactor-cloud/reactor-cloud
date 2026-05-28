//! Bundle upload handling.

use crate::error::SitesError;

/// Upload result tracking.
#[derive(Debug, Default)]
pub struct UploadStats {
    /// Number of files uploaded.
    pub file_count: u32,
    /// Total bytes uploaded.
    pub total_bytes: u64,
}

impl UploadStats {
    /// Create new upload stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file to the stats.
    pub fn add_file(&mut self, bytes: u64) {
        self.file_count += 1;
        self.total_bytes += bytes;
    }
}

/// Validate bundle size limits.
pub fn validate_bundle_limits(
    file_count: u32,
    total_bytes: u64,
    max_files: u32,
    max_bytes: u64,
) -> Result<(), SitesError> {
    if file_count > max_files {
        return Err(SitesError::BundleInvalid(format!(
            "too many files: {} (max {})",
            file_count, max_files
        )));
    }

    if total_bytes > max_bytes {
        return Err(SitesError::BundleTooLarge {
            max: max_bytes,
            actual: total_bytes,
        });
    }

    Ok(())
}
