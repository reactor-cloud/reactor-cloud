// Ported from 1jehuang/jcode (MIT) - src/tool/bash.rs
// Adapted for Reactor Studio.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::{Tool, ToolContext, ToolError, ToolResult};

/// Tool for executing shell commands
pub struct BashTool {
    approval_required: AtomicBool,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            approval_required: AtomicBool::new(true),
        }
    }

    pub fn with_auto_approve(self) -> Self {
        self.approval_required.store(false, Ordering::SeqCst);
        self
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct BashArgs {
    command: String,
    #[serde(default)]
    working_directory: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command. Use for git, npm, build tools, etc. Avoid for file operations - use dedicated tools instead."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["command"],
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "working_directory": {
                    "type": "string",
                    "description": "Working directory for the command (defaults to workspace root)"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 60)"
                }
            }
        })
    }

    fn requires_approval(&self) -> bool {
        self.approval_required.load(Ordering::SeqCst)
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: BashArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgument(e.to_string()))?;

        let working_dir = args
            .working_directory
            .map(|p| {
                let path = PathBuf::from(&p);
                if path.is_absolute() {
                    path
                } else {
                    PathBuf::from(&ctx.workspace_path).join(p)
                }
            })
            .unwrap_or_else(|| PathBuf::from(&ctx.workspace_path));

        let output = Command::new("sh")
            .arg("-c")
            .arg(&args.command)
            .current_dir(&working_dir)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let combined = if stderr.is_empty() {
            stdout.to_string()
        } else if stdout.is_empty() {
            stderr.to_string()
        } else {
            format!("{}\n{}", stdout, stderr)
        };

        if output.status.success() {
            Ok(ToolResult::success(if combined.is_empty() {
                "Command completed successfully with no output.".to_string()
            } else {
                combined
            }))
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            Ok(ToolResult::error(format!(
                "Command failed with exit code {}:\n{}",
                exit_code, combined
            )))
        }
    }
}
