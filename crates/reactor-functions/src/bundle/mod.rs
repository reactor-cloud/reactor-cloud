//! Bundle handling for function deployments.
//!
//! A bundle is a zip file containing:
//! - `manifest.json` - function configuration
//! - `code/` - runtime-specific code files

mod manifest;

pub use manifest::{
    BundleLimits, ConcurrencyConfig, JobBackoffStrategy, JobConfig, JobRetryConfig,
    JobTriggerConfig, JobTriggerKind, Manifest, RuntimeKind, BUNDLE_MAX_SIZE,
    DEFAULT_MAX_BODY_IN_MB, DEFAULT_MAX_BODY_OUT_MB, DEFAULT_MAX_CONCURRENCY,
    DEFAULT_MEMORY_MB, DEFAULT_MIN_INSTANCES, DEFAULT_TIMEOUT_MS,
};

use crate::error::FunctionsError;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek};
use zip::ZipArchive;

/// System bucket name for function bundles.
pub const SYSTEM_BUCKET: &str = "_reactor_functions";

/// Validate and extract a manifest from a bundle zip file.
pub fn extract_manifest<R: Read + Seek>(reader: R) -> Result<Manifest, FunctionsError> {
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| FunctionsError::BundleInvalid(format!("invalid zip file: {}", e)))?;

    // Find manifest.json
    let mut manifest_file = archive.by_name("manifest.json").map_err(|_| {
        FunctionsError::BundleInvalid("manifest.json not found in bundle".to_string())
    })?;

    let mut manifest_contents = String::new();
    manifest_file
        .read_to_string(&mut manifest_contents)
        .map_err(|e| FunctionsError::BundleInvalid(format!("failed to read manifest.json: {}", e)))?;

    let manifest: Manifest = serde_json::from_str(&manifest_contents)
        .map_err(|e| FunctionsError::ManifestInvalid(format!("invalid manifest JSON: {}", e)))?;

    Ok(manifest)
}

/// Validate that the bundle contains the expected code directory.
pub fn validate_bundle_structure<R: Read + Seek>(reader: R) -> Result<(), FunctionsError> {
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| FunctionsError::BundleInvalid(format!("invalid zip file: {}", e)))?;

    // Check for manifest.json
    archive.by_name("manifest.json").map_err(|_| {
        FunctionsError::BundleInvalid("manifest.json not found in bundle".to_string())
    })?;

    // Check for code directory (at least one file starting with code/)
    let has_code = (0..archive.len()).any(|i| {
        archive
            .by_index(i)
            .map(|f| f.name().starts_with("code/"))
            .unwrap_or(false)
    });

    if !has_code {
        return Err(FunctionsError::BundleInvalid(
            "bundle must contain a code/ directory".to_string(),
        ));
    }

    Ok(())
}

/// Compute SHA256 hash of bundle data.
pub fn compute_sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Verify bundle SHA256 matches expected hash.
pub fn verify_sha256(data: &[u8], expected: &[u8]) -> Result<(), FunctionsError> {
    let actual = compute_sha256(data);
    if actual != expected {
        return Err(FunctionsError::BundleInvalid(
            "SHA256 hash mismatch".to_string(),
        ));
    }
    Ok(())
}

/// Generate the storage object key for a bundle.
pub fn bundle_object_key(function_name: &str, version: i64) -> String {
    format!("{}/{}.zip", function_name, version)
}

/// Generate the storage object key for a manifest.
pub fn manifest_object_key(function_name: &str, version: i64) -> String {
    format!("{}/manifests/{}.json", function_name, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sha256() {
        let data = b"hello world";
        let hash = compute_sha256(data);
        assert_eq!(hash.len(), 32);

        // Same input should produce same hash
        let hash2 = compute_sha256(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_verify_sha256() {
        let data = b"hello world";
        let expected = compute_sha256(data);

        // Correct hash should pass
        assert!(verify_sha256(data, &expected).is_ok());

        // Wrong hash should fail
        let wrong = compute_sha256(b"wrong data");
        assert!(verify_sha256(data, &wrong).is_err());
    }

    #[test]
    fn test_bundle_object_key() {
        assert_eq!(bundle_object_key("my-func", 1), "my-func/1.zip");
        assert_eq!(bundle_object_key("my-func", 42), "my-func/42.zip");
    }

    #[test]
    fn test_manifest_object_key() {
        assert_eq!(
            manifest_object_key("my-func", 1),
            "my-func/manifests/1.json"
        );
    }
}
