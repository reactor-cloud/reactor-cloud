// Ported from 1jehuang/jcode (MIT) - jcode-storage/src/error.rs
// Adapted for Reactor Studio.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}
