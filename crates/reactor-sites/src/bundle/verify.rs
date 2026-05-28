//! Bundle verification utilities.

use sha2::{Digest, Sha256};

/// Compute SHA256 hash of data.
pub fn compute_sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Verify SHA256 hash matches expected.
pub fn verify_sha256(data: &[u8], expected: &[u8]) -> bool {
    let actual = compute_sha256(data);
    actual == expected
}

/// Convert SHA256 hash to hex string.
pub fn sha256_to_hex(hash: &[u8]) -> String {
    hex::encode(hash)
}
