//! Bundle unpacking utilities.

use crate::error::BundleError;
use crate::manifest::BundleManifest;
use std::io::Read;
use std::path::{Path, PathBuf};

/// An unpacked bundle with typed access to its contents.
#[derive(Debug)]
pub struct Bundle {
    /// The parsed manifest.
    pub manifest: BundleManifest,

    /// Root directory where the bundle was unpacked.
    pub root: PathBuf,
}

impl Bundle {
    /// Get the path to a file within the bundle.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// Read a file from the bundle.
    pub fn read_file(&self, relative: &str) -> Result<Vec<u8>, BundleError> {
        let path = self.path(relative);
        if !path.exists() {
            return Err(BundleError::FileNotFound(relative.to_string()));
        }
        std::fs::read(&path).map_err(BundleError::Io)
    }
}

/// Unpack a deploy bundle to a destination directory.
///
/// Returns a `Bundle` handle with the parsed manifest and root path.
/// Does NOT verify hashes — call `validate()` separately if needed.
pub fn unpack(data: &[u8], dest: &Path) -> Result<Bundle, BundleError> {
    // Decompress zstd
    let mut decoder = zstd::Decoder::new(data)
        .map_err(|e| BundleError::Decompression(e.to_string()))?;
    let mut tar_data = Vec::new();
    decoder
        .read_to_end(&mut tar_data)
        .map_err(|e| BundleError::Decompression(e.to_string()))?;

    // Extract tar
    let mut archive = tar::Archive::new(tar_data.as_slice());
    archive.unpack(dest)?;

    // Read and parse manifest
    let manifest_path = dest.join("manifest.json");
    if !manifest_path.exists() {
        return Err(BundleError::MissingManifest);
    }

    let manifest_data = std::fs::read_to_string(&manifest_path)?;
    let manifest: BundleManifest = serde_json::from_str(&manifest_data)?;

    Ok(Bundle {
        manifest,
        root: dest.to_path_buf(),
    })
}
