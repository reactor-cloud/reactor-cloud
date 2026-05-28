// Task-related tools for agents
// These tools allow agents to interact with the task system

use serde_json::{json, Value};

use crate::{ToolContext, ToolError, ToolResult, ToolDefinition};

/// Tool to signal that a phase is ready to advance
pub struct TaskAdvanceTool;

impl TaskAdvanceTool {
    pub fn definition() -> ToolDefinition {
        ToolDefinition {
            name: "task_advance".to_string(),
            description: "Signal that the current task phase is complete and ready to advance to the next phase. Call this when you have gathered enough information or completed the work for the current phase.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "The ID of the task to advance"
                    },
                    "summary": {
                        "type": "string",
                        "description": "A brief summary of what was accomplished in this phase"
                    }
                },
                "required": ["task_id", "summary"]
            }),
        }
    }

    pub async fn execute(args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let task_id = args["task_id"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgument("task_id is required".into()))?;
        let summary = args["summary"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgument("summary is required".into()))?;

        // This would normally call into the task store, but since we're in a tool context
        // we emit an event that the IPC layer will handle
        Ok(ToolResult::success(format!(
            "Phase advance requested for task {}. Summary: {}",
            task_id, summary
        )))
    }
}

/// Tool to write an artifact (like a plan or test report) for a task
pub struct TaskArtifactWriteTool;

impl TaskArtifactWriteTool {
    pub fn definition() -> ToolDefinition {
        ToolDefinition {
            name: "task_artifact_write".to_string(),
            description: "Write an artifact file for the current task, such as a plan document or test report.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "The ID of the task"
                    },
                    "artifact_type": {
                        "type": "string",
                        "enum": ["plan", "test_report", "deployment_notes"],
                        "description": "The type of artifact to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The markdown content of the artifact"
                    }
                },
                "required": ["task_id", "artifact_type", "content"]
            }),
        }
    }

    pub async fn execute(args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let task_id = args["task_id"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgument("task_id is required".into()))?;
        let artifact_type = args["artifact_type"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgument("artifact_type is required".into()))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgument("content is required".into()))?;

        // Write artifact to .reactor/tasks/{task_id}/{artifact_type}.md
        let artifact_path = std::path::Path::new(&ctx.workspace_path)
            .join(".reactor")
            .join("tasks")
            .join(task_id)
            .join(format!("{}.md", artifact_type));

        if let Some(parent) = artifact_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&artifact_path, content)?;

        Ok(ToolResult::success(format!(
            "Wrote {} artifact for task {} to {}",
            artifact_type,
            task_id,
            artifact_path.display()
        )))
    }
}
