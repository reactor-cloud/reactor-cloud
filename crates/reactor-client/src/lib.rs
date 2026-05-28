//! HTTP client for Reactor server APIs.
//!
//! This crate provides a typed client for interacting with Reactor servers,
//! supporting both admin endpoints (`/_admin/*`) and per-capability admin
//! endpoints (`/auth/v1/_admin/*`, `/fn/v1/_admin/*`, etc.).

pub mod admin;
pub mod ai;
pub mod auth;
pub mod cloud;
pub mod connect;
pub mod data;
pub mod error;
pub mod functions;
pub mod http;
pub mod jobs;
pub mod sites;
pub mod storage;

pub use error::{ClientError, ClientResult};
pub use http::{Client, ClientConfig};

/// Standard API response envelope for success.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiSuccess<T> {
    pub ok: bool,
    pub data: T,
}

/// Standard API response envelope for errors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiError {
    pub ok: bool,
    pub error: ApiErrorDetail,
}

/// Error detail within the API error envelope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: ApiErrorDetail {
                code: code.into(),
                message: message.into(),
                hint: None,
            },
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.error.hint = Some(hint.into());
        self
    }
}

impl<T> ApiSuccess<T> {
    pub fn new(data: T) -> Self {
        Self { ok: true, data }
    }
}
