//! Integration tests for deploy bundle pack/unpack/validate.

use reactor_deploy_bundle::{
    pack, unpack, validate, BundleError, BundleManifest, CapabilitiesManifest, DataManifest,
    FunctionEntry, MigrationEntry,
};
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::TempDir;

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn create_test_bundle_dir() -> (TempDir, BundleManifest) {
    let dir = TempDir::new().unwrap();

    // Create migration file
    let migrations_dir = dir.path().join("migrations").join("data");
    fs::create_dir_all(&migrations_dir).unwrap();
    let migration_content = b"CREATE TABLE users (id UUID PRIMARY KEY);";
    let migration_path = migrations_dir.join("001_init.sql");
    fs::write(&migration_path, migration_content).unwrap();

    // Create function bundle
    let functions_dir = dir.path().join("functions");
    fs::create_dir_all(&functions_dir).unwrap();
    let function_content = b"module.exports = { handler: () => {} };";
    let function_path = functions_dir.join("hello.tar.zst");
    fs::write(&function_path, function_content).unwrap();

    // Create manifest
    let manifest = BundleManifest {
        project_id: "proj_test123".to_string(),
        reactor_version: "0.1.0".to_string(),
        created_at: "2026-05-15T00:00:00Z".to_string(),
        capabilities: CapabilitiesManifest {
            data: Some(DataManifest {
                migrations: vec![MigrationEntry {
                    name: "001_init.sql".to_string(),
                    path: "migrations/data/001_init.sql".to_string(),
                    sha256: sha256_hex(migration_content),
                }],
            }),
            storage: None,
            functions: Some(vec![FunctionEntry {
                name: "hello".to_string(),
                path: "functions/hello.tar.zst".to_string(),
                sha256: sha256_hex(function_content),
                runtime: "wasm".to_string(),
            }]),
            jobs: None,
            sites: None,
        },
    };

    // Write manifest
    let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
    fs::write(dir.path().join("manifest.json"), &manifest_json).unwrap();

    (dir, manifest)
}

#[test]
fn test_pack_unpack_roundtrip() {
    let (src_dir, original_manifest) = create_test_bundle_dir();

    // Pack
    let bundle_data = pack(src_dir.path()).expect("pack should succeed");
    assert!(!bundle_data.is_empty());

    // Unpack to new location
    let dest_dir = TempDir::new().unwrap();
    let bundle = unpack(&bundle_data, dest_dir.path()).expect("unpack should succeed");

    // Verify manifest matches
    assert_eq!(bundle.manifest.project_id, original_manifest.project_id);
    assert_eq!(
        bundle.manifest.reactor_version,
        original_manifest.reactor_version
    );

    // Verify files exist
    assert!(bundle.path("manifest.json").exists());
    assert!(bundle.path("migrations/data/001_init.sql").exists());
    assert!(bundle.path("functions/hello.tar.zst").exists());
}

#[test]
fn test_validate_success() {
    let (src_dir, _) = create_test_bundle_dir();
    let bundle_data = pack(src_dir.path()).unwrap();

    let dest_dir = TempDir::new().unwrap();
    let bundle = unpack(&bundle_data, dest_dir.path()).unwrap();

    // Validate without version check
    validate(&bundle, None).expect("validate should succeed");

    // Validate with compatible version
    validate(&bundle, Some("0.1.0")).expect("validate should succeed with same version");
    validate(&bundle, Some("0.2.0")).expect("validate should succeed with newer version");
}

#[test]
fn test_validate_version_mismatch() {
    let (src_dir, _) = create_test_bundle_dir();
    let bundle_data = pack(src_dir.path()).unwrap();

    let dest_dir = TempDir::new().unwrap();
    let bundle = unpack(&bundle_data, dest_dir.path()).unwrap();

    // Server is older than required version
    let result = validate(&bundle, Some("0.0.9"));
    assert!(matches!(result, Err(BundleError::VersionMismatch { .. })));
}

#[test]
fn test_validate_hash_mismatch() {
    let (src_dir, _) = create_test_bundle_dir();
    let bundle_data = pack(src_dir.path()).unwrap();

    let dest_dir = TempDir::new().unwrap();
    let bundle = unpack(&bundle_data, dest_dir.path()).unwrap();

    // Tamper with a file
    fs::write(
        bundle.path("migrations/data/001_init.sql"),
        b"TAMPERED CONTENT",
    )
    .unwrap();

    let result = validate(&bundle, None);
    assert!(matches!(result, Err(BundleError::HashMismatch { .. })));
}

#[test]
fn test_pack_missing_manifest() {
    let dir = TempDir::new().unwrap();
    // Don't create manifest.json

    let result = pack(dir.path());
    assert!(matches!(result, Err(BundleError::MissingManifest)));
}

#[test]
fn test_unpack_corrupted_bundle() {
    let dest_dir = TempDir::new().unwrap();
    let corrupted_data = b"not a valid zstd archive";

    let result = unpack(corrupted_data, dest_dir.path());
    assert!(matches!(result, Err(BundleError::Decompression(_))));
}

#[test]
fn test_manifest_serialization() {
    let manifest = BundleManifest {
        project_id: "proj_abc".to_string(),
        reactor_version: "0.1.0".to_string(),
        created_at: "2026-05-15T12:00:00Z".to_string(),
        capabilities: CapabilitiesManifest::default(),
    };

    let json = serde_json::to_string(&manifest).unwrap();
    let parsed: BundleManifest = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.project_id, manifest.project_id);
    assert_eq!(parsed.reactor_version, manifest.reactor_version);
}

#[test]
fn test_bundle_read_file() {
    let (src_dir, _) = create_test_bundle_dir();
    let bundle_data = pack(src_dir.path()).unwrap();

    let dest_dir = TempDir::new().unwrap();
    let bundle = unpack(&bundle_data, dest_dir.path()).unwrap();

    // Read existing file
    let content = bundle.read_file("migrations/data/001_init.sql").unwrap();
    assert_eq!(content, b"CREATE TABLE users (id UUID PRIMARY KEY);");

    // Read non-existent file
    let result = bundle.read_file("nonexistent.txt");
    assert!(matches!(result, Err(BundleError::FileNotFound(_))));
}
