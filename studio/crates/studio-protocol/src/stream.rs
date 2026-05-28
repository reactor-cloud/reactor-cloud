// Ported from 1jehuang/jcode (MIT) - jcode-protocol/src/stream.rs
// Adapted for Reactor Studio.

use serde::{Deserialize, Serialize};
use crate::message::ToolCall;

/// Chunk types emitted during streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
    /// Thinking/reasoning tokens (Claude-style)
    Thinking { content: String },
    
    /// Regular text content
    Text { content: String },
    
    /// Tool call being initiated
    ToolCallStart { 
        id: String, 
        name: String 
    },
    
    /// Tool call arguments (may be streamed in parts)
    #[serde(rename_all = "camelCase")]
    ToolCallDelta { 
        id: String, 
        arguments_delta: String 
    },
    
    /// Complete tool call
    ToolCall(ToolCall),
    
    /// Tool execution result
    #[serde(rename_all = "camelCase")]
    ToolResult { 
        tool_call_id: String, 
        output: String, 
        is_error: bool 
    },
    
    /// Error during streaming
    Error { message: String },
    
    /// Stream complete
    #[serde(rename_all = "camelCase")]
    Done { 
        #[serde(skip_serializing_if = "Option::is_none")]
        finish_reason: Option<String> 
    },
}

impl StreamChunk {
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text { content: content.into() }
    }

    pub fn thinking(content: impl Into<String>) -> Self {
        Self::Thinking { content: content.into() }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error { message: message.into() }
    }

    pub fn done() -> Self {
        Self::Done { finish_reason: None }
    }

    pub fn done_with_reason(reason: impl Into<String>) -> Self {
        Self::Done { finish_reason: Some(reason.into()) }
    }
}
