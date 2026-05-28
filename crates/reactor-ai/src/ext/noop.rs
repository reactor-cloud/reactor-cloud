//! No-op implementation of AI extensions.

use async_trait::async_trait;

use super::{AiExtensions, RequestCtx, UsageEvent};
use crate::error::AiError;

/// No-op implementation of `AiExtensions`.
///
/// This is the default implementation for the open-source version.
/// All hooks do nothing and return success.
#[derive(Debug, Clone, Default)]
pub struct NoopExtensions;

impl NoopExtensions {
    /// Create a new no-op extensions instance.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AiExtensions for NoopExtensions {
    async fn pre_request(&self, _ctx: &RequestCtx) -> Result<(), AiError> {
        Ok(())
    }

    async fn post_usage(&self, _event: &UsageEvent) -> Result<(), AiError> {
        Ok(())
    }

    fn route_override(&self, _ctx: &RequestCtx) -> Option<String> {
        None
    }
}
