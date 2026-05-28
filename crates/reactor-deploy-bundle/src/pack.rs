//! Bundle packing utilities.

use crate::error::BundleError;
use std::io::Write;
use std::path::Path;

/// Pack a directory into a deploy bundle (tar.zst).
///
/// The directory must contain a `manifest.json` at the root.
/// Files are added in deterministic (sorted) order for reproducibility.
pub fn pack(dir: &Path) -> Result<Vec<u8>, BundleError> {
    let manifest_path = dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(BundleError::MissingManifest);
    }

    // Collect all files in sorted order for deterministic output
    let mut entries: Vec<_> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();
    entries.sort_by(|a, b| a.path().cmp(b.path()));

    // Create tar archive in memory
    let mut tar_data = Vec::new();
    {
        let mut tar_builder = tar::Builder::new(&mut tar_data);

        for entry in entries {
            let path = entry.path();
            let rel_path = path
                .strip_prefix(dir)
                .map_err(|e| BundleError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

            let mut file = std::fs::File::open(path)?;
            tar_builder.append_file(rel_path, &mut file)?;
        }

        tar_builder.finish()?;
    }

    // Compress with zstd
    let mut encoder = zstd::Encoder::new(Vec::new(), 3)
        .map_err(|e| BundleError::Compression(e.to_string()))?;
    encoder
        .write_all(&tar_data)
        .map_err(|e| BundleError::Compression(e.to_string()))?;
    let compressed = encoder
        .finish()
        .map_err(|e| BundleError::Compression(e.to_string()))?;

    Ok(compressed)
}

// Note: walkdir is needed for recursive directory traversal
// We'll add it to dependencies
