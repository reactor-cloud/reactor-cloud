//! Extension hooks for reactor.cloud integration.
//!
//! This module provides the `AiExtensions` trait that allows the closed-source
//! reactor.cloud offering to inject quota checks, billing, and region routing
//! without modifying the open-source codebase.

mod noop;

pub use noop::NoopExtensions;

use async_trait::async_trait;

use crate::error::AiError;

/// Context for pre-request hooks.
#[derive(Debug, Clone)]
pub struct RequestCtx {
    /// Resolved model ID.
    pub model_id: String,
    /// User ID from reactor-auth (if authenticated as a user).
    pub user_id: Option<String>,
}

/// Usage event for post-request hooks.
#[derive(Debug, Clone)]
pub struct UsageEvent {
    /// Model ID used.
    pub model_id: String,
    /// User ID from reactor-auth (if authenticated as a user).
    pub user_id: Option<String>,
    /// Input tokens.
    pub tokens_in: u32,
    /// Output tokens.
    pub tokens_out: u32,
}

/// Extension trait for reactor.cloud integration.
///
/// The open-source version ships with `NoopExtensions` which does nothing.
/// The closed-source reactor.cloud provides a richer implementation with:
/// - Per-user quota enforcement
/// - Credit balance checks
/// - Billing event emission
/// - Region-pinned routing
#[async_trait]
pub trait AiExtensions: Send + Sync {
    /// Called before a request is dispatched.
    ///
    /// Use this to check quotas, balances, and region constraints.
    /// Return an error to reject the request.
    async fn pre_request(&self, ctx: &RequestCtx) -> Result<(), AiError>;

    /// Called after a request completes with usage data.
    ///
    /// Use this to emit billing events, update usage counters, and trigger alerts.
    async fn post_usage(&self, event: &UsageEvent) -> Result<(), AiError>;

    /// Called to determine if the request should be routed to a specific upstream.
    ///
    /// Return `Some(url)` to override the default provider routing.
    /// This is used for region-pinned deployments.
    fn route_override(&self, ctx: &RequestCtx) -> Option<String>;
}
