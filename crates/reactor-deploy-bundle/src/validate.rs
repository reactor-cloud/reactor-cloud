//! Bundle validation utilities.

use crate::error::BundleError;
use crate::unpack::Bundle;
use sha2::{Digest, Sha256};

/// Validate a bundle's contents against its manifest.
///
/// Checks:
/// - All referenced files exist
/// - All SHA-256 hashes match
/// - Reactor version compatibility (if `server_version` provided)
pub fn validate(bundle: &Bundle, server_version: Option<&str>) -> Result<(), BundleError> {
    // Check version compatibility if server version provided
    if let Some(server_ver) = server_version {
        let required: semver::Version = bundle
            .manifest
            .reactor_version
            .parse()
            .map_err(|e| BundleError::InvalidManifest(format!("invalid version: {}", e)))?;

        let actual: semver::Version = server_ver
            .parse()
            .map_err(|e| BundleError::InvalidManifest(format!("invalid server version: {}", e)))?;

        // Bundle's required version must be <= server version
        if required > actual {
            return Err(BundleError::VersionMismatch {
                required: required.to_string(),
                actual: actual.to_string(),
            });
        }
    }

    // Validate data migrations
    if let Some(ref data) = bundle.manifest.capabilities.data {
        for migration in &data.migrations {
            verify_file_hash(bundle, &migration.path, &migration.sha256)?;
        }
    }

    // Validate storage policies
    if let Some(ref policies) = bundle.manifest.capabilities.storage {
        for policy in policies {
            verify_file_hash(bundle, &policy.path, &policy.sha256)?;
        }
    }

    // Validate functions (skip if sha256 is empty - directory entries)
    if let Some(ref functions) = bundle.manifest.capabilities.functions {
        for func in functions {
            if !func.sha256.is_empty() {
                verify_file_hash(bundle, &func.path, &func.sha256)?;
            } else {
                // Just verify the directory exists
                let path = bundle.path(&func.path);
                if !path.exists() {
                    return Err(BundleError::FileNotFound(func.path.clone()));
                }
            }
        }
    }

    // Validate jobs
    if let Some(ref jobs) = bundle.manifest.capabilities.jobs {
        for job in jobs {
            if !job.sha256.is_empty() {
                verify_file_hash(bundle, &job.path, &job.sha256)?;
            } else {
                let path = bundle.path(&job.path);
                if !path.exists() {
                    return Err(BundleError::FileNotFound(job.path.clone()));
                }
            }
        }
    }

    // Validate sites (skip hash check if empty - directory entries)
    if let Some(ref sites) = bundle.manifest.capabilities.sites {
        for site in sites {
            if !site.sha256.is_empty() {
                verify_file_hash(bundle, &site.path, &site.sha256)?;
            } else {
                // Just verify the directory exists
                let path = bundle.path(&site.path);
                if !path.exists() {
                    return Err(BundleError::FileNotFound(site.path.clone()));
                }
            }
        }
    }

    Ok(())
}

fn verify_file_hash(bundle: &Bundle, path: &str, expected_hash: &str) -> Result<(), BundleError> {
    let data = bundle.read_file(path)?;

    let mut hasher = Sha256::new();
    hasher.update(&data);
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != expected_hash {
        return Err(BundleError::HashMismatch {
            path: path.to_string(),
            expected: expected_hash.to_string(),
            actual: actual_hash,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_version_comparison() {
        let v1: semver::Version = "0.1.0".parse().unwrap();
        let v2: semver::Version = "0.2.0".parse().unwrap();
        assert!(v1 < v2);
    }
}
