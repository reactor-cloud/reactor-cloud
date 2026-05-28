// Ported from 1jehuang/jcode (MIT) - jcode-provider-core/src/error.rs
// Adapted for Reactor Studio.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Rate limit exceeded")]
    RateLimited,

    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Provider not configured")]
    NotConfigured,
}
