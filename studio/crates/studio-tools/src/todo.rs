// Ported from 1jehuang/jcode (MIT) - src/tool/todo.rs
// Adapted for Reactor Studio.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::{Tool, ToolContext, ToolError, ToolResult};

/// A todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

/// Tool for managing a todo list within a conversation
pub struct TodoTool {
    todos: Mutex<HashMap<String, Vec<TodoItem>>>,
}

impl TodoTool {
    pub fn new() -> Self {
        Self {
            todos: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct TodoArgs {
    action: TodoAction,
    #[serde(default)]
    todos: Option<Vec<TodoItem>>,
    #[serde(default)]
    merge: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum TodoAction {
    Write,
    Read,
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Manage a todo list for tracking progress on complex tasks. Use 'write' to create/update todos, 'read' to view current list."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["write", "read"],
                    "description": "Action to perform: 'write' to update todos, 'read' to view"
                },
                "todos": {
                    "type": "array",
                    "description": "List of todo items (required for write action)",
                    "items": {
                        "type": "object",
                        "required": ["id", "content", "status"],
                        "properties": {
                            "id": { "type": "string" },
                            "content": { "type": "string" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "cancelled"]
                            }
                        }
                    }
                },
                "merge": {
                    "type": "boolean",
                    "description": "If true, merge with existing todos. If false, replace all."
                }
            }
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: TodoArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgument(e.to_string()))?;

        let mut todos = self.todos.lock().unwrap();
        let conversation_todos = todos.entry(ctx.conversation_id.clone()).or_default();

        match args.action {
            TodoAction::Write => {
                let new_todos = args.todos.ok_or_else(|| {
                    ToolError::InvalidArgument("todos array required for write action".to_string())
                })?;

                if args.merge.unwrap_or(true) {
                    // Merge: update existing, add new
                    for new_todo in new_todos {
                        if let Some(existing) = conversation_todos
                            .iter_mut()
                            .find(|t| t.id == new_todo.id)
                        {
                            existing.content = new_todo.content;
                            existing.status = new_todo.status;
                        } else {
                            conversation_todos.push(new_todo);
                        }
                    }
                } else {
                    // Replace
                    *conversation_todos = new_todos;
                }

                Ok(ToolResult::success(format!(
                    "Updated {} todo items.",
                    conversation_todos.len()
                )))
            }
            TodoAction::Read => {
                if conversation_todos.is_empty() {
                    Ok(ToolResult::success("No todos in current conversation."))
                } else {
                    let output = conversation_todos
                        .iter()
                        .map(|t| {
                            let status_icon = match t.status {
                                TodoStatus::Pending => "⬜",
                                TodoStatus::InProgress => "🔄",
                                TodoStatus::Completed => "✅",
                                TodoStatus::Cancelled => "❌",
                            };
                            format!("{} [{}] {}", status_icon, t.id, t.content)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    Ok(ToolResult::success(output))
                }
            }
        }
    }
}
