// Ported from 1jehuang/jcode (MIT) - src/tool/error.rs
// Adapted for Reactor Studio.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Requires approval")]
    RequiresApproval,

    #[error("Cancelled")]
    Cancelled,
}
