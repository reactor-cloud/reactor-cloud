//! Error types for the reactor-server crate.

use thiserror::Error;

/// Errors that can occur in the reactor-server.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Migration error.
    #[error("migration error: {0}")]
    Migration(String),

    /// Capability error.
    #[error("{capability} error: {message}")]
    Capability {
        /// Name of the capability that failed.
        capability: String,
        /// Error message.
        message: String,
    },

    /// Boot error.
    #[error("boot error: {0}")]
    Boot(String),

    /// Shutdown error.
    #[error("shutdown error: {0}")]
    Shutdown(String),

    /// Admin endpoint error.
    #[error("admin error: {0}")]
    Admin(String),

    /// Deploy error.
    #[error("deploy error: {0}")]
    Deploy(String),

    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
