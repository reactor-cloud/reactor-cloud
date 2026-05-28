//! Action execution logic.
//!
//! This module handles action invocation, dry-run synthesis, and idempotency.

use crate::error::ConnectError;
use crate::protocol::ActionResult;

/// Action executor.
pub struct ActionExecutor {
    // Future: rate limiter, circuit breaker, etc.
}

impl ActionExecutor {
    /// Create a new executor.
    pub fn new() -> Self {
        Self {}
    }

    /// Execute an action.
    pub async fn execute(
        &self,
        _runtime: &dyn crate::runtime::ConnectorRuntime,
        _type_id: &str,
        _config: &serde_json::Value,
        _action: &str,
        _input: &serde_json::Value,
        _opts: &crate::runtime::ActionOpts,
    ) -> Result<ActionResult, ConnectError> {
        // TODO: Implement with rate limiting and circuit breaking
        Err(ConnectError::Internal("not implemented".to_string()))
    }
}

impl Default for ActionExecutor {
    fn default() -> Self {
        Self::new()
    }
}
