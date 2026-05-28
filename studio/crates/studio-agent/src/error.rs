// Ported from 1jehuang/jcode (MIT) - jcode-agent-runtime/src/error.rs
// Adapted for Reactor Studio.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Provider error: {0}")]
    Provider(#[from] studio_providers::ProviderError),

    #[error("Storage error: {0}")]
    Storage(#[from] studio_storage::StorageError),

    #[error("Tool error: {0}")]
    Tool(#[from] studio_tools::ToolError),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Conversation not found: {0}")]
    ConversationNotFound(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("Max iterations exceeded")]
    MaxIterations,

    #[error("Internal error: {0}")]
    Internal(String),
}
