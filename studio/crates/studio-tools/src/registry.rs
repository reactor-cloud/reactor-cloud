// Ported from 1jehuang/jcode (MIT) - src/tool/registry.rs
// Adapted for Reactor Studio.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::ToolError;

/// Result of executing a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            output: message.into(),
            is_error: true,
        }
    }
}

/// Tool definition for registration
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Context passed to tool execution
pub struct ToolContext {
    pub workspace_path: String,
    pub conversation_id: String,
}

/// Trait for tools
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError>;

    fn requires_approval(&self) -> bool {
        false
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }
}

/// Registry for all available tools
#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn list(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        args: Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let tool = self.get(name).ok_or_else(|| {
            ToolError::NotFound(format!("Tool not found: {}", name))
        })?;

        tool.execute(args, ctx).await
    }

    /// Create a registry with default tools
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        
        registry.register(Arc::new(crate::FileReadTool));
        registry.register(Arc::new(crate::FileWriteTool));
        registry.register(Arc::new(crate::FileEditTool));
        registry.register(Arc::new(crate::GrepTool));
        registry.register(Arc::new(crate::GlobTool));
        registry.register(Arc::new(crate::BashTool::new()));
        registry.register(Arc::new(crate::TodoTool::new()));
        
        registry
    }
}
