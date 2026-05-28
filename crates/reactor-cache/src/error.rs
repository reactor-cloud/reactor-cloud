//! Cache error types.

use thiserror::Error;

/// Errors that can occur in cache operations.
#[derive(Debug, Error)]
pub enum CacheError {
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Item not found.
    #[error("item not found: {0}")]
    NotFound(String),

    /// Invalid receipt.
    #[error("invalid receipt: {0}")]
    InvalidReceipt(String),

    /// Queue is empty.
    #[error("queue is empty")]
    QueueEmpty,

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}
