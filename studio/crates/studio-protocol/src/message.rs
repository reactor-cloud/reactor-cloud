// Ported from 1jehuang/jcode (MIT) - jcode-message-types/src/lib.rs
// Adapted for Reactor Studio.

use serde::{Deserialize, Serialize};
use crate::types::{MessageId, Role};

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(tool_calls);
        self
    }
}

/// A tool call requested by the assistant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

impl ToolCall {
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

/// Result of executing a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub output: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(tool_call_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            output: output.into(),
            is_error: false,
        }
    }

    pub fn error(tool_call_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            output: error.into(),
            is_error: true,
        }
    }
}
