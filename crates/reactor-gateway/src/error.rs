//! Error types for the gateway crate.

use thiserror::Error;

/// Gateway operation result type.
pub type GatewayResult<T> = Result<T, GatewayError>;

/// Gateway errors.
#[derive(Debug, Error)]
pub enum GatewayError {
    /// Database error.
    #[cfg(feature = "postgres")]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Caddy admin API error.
    #[error("Caddy admin API error: {0}")]
    CaddyAdmin(String),

    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Route not found.
    #[error("Route not found for host: {0}")]
    RouteNotFound(String),

    /// Domain not verified.
    #[error("Domain not verified: {0}")]
    DomainNotVerified(String),

    /// DNS verification failed.
    #[error("DNS verification failed: {0}")]
    DnsVerificationFailed(String),

    /// Snapshot rollback error.
    #[error("Snapshot rollback failed: {0}")]
    RollbackFailed(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl GatewayError {
    /// Create a new Caddy admin error.
    pub fn caddy_admin(msg: impl Into<String>) -> Self {
        Self::CaddyAdmin(msg.into())
    }

    /// Create a new configuration error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a new internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}
