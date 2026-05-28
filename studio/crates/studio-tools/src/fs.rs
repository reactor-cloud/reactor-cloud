// Ported from 1jehuang/jcode (MIT) - src/tool/{read,write,edit}.rs
// Adapted for Reactor Studio.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::{Tool, ToolContext, ToolError, ToolResult};

/// Tool for reading file contents
pub struct FileReadTool;

#[derive(Deserialize)]
struct FileReadArgs {
    path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Use offset and limit for large files."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file (relative to workspace or absolute)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-indexed)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                }
            }
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: FileReadArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgument(e.to_string()))?;

        let path = resolve_path(&args.path, &ctx.workspace_path);

        if !path.exists() {
            return Err(ToolError::NotFound(format!(
                "File not found: {}",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(&path)?;

        let output = if args.offset.is_some() || args.limit.is_some() {
            let lines: Vec<&str> = content.lines().collect();
            let offset = args.offset.unwrap_or(1).saturating_sub(1);
            let limit = args.limit.unwrap_or(lines.len());

            lines
                .iter()
                .skip(offset)
                .take(limit)
                .enumerate()
                .map(|(i, line)| format!("{:6}|{}", offset + i + 1, line))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            content
                .lines()
                .enumerate()
                .map(|(i, line)| format!("{:6}|{}", i + 1, line))
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(ToolResult::success(output))
    }
}

/// Tool for writing file contents
pub struct FileWriteTool;

#[derive(Deserialize)]
struct FileWriteArgs {
    path: String,
    contents: String,
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write contents to a file. Creates the file if it doesn't exist."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "contents"],
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file (relative to workspace or absolute)"
                },
                "contents": {
                    "type": "string",
                    "description": "Contents to write to the file"
                }
            }
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: FileWriteArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgument(e.to_string()))?;

        let path = resolve_path(&args.path, &ctx.workspace_path);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, &args.contents)?;

        Ok(ToolResult::success(format!(
            "Successfully wrote {} bytes to {}",
            args.contents.len(),
            path.display()
        )))
    }
}

/// Tool for editing files with search/replace
pub struct FileEditTool;

#[derive(Deserialize)]
struct FileEditArgs {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing old_string with new_string. The old_string must be unique unless replace_all is true."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "old_string", "new_string"],
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The string to replace it with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)"
                }
            }
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: FileEditArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgument(e.to_string()))?;

        let path = resolve_path(&args.path, &ctx.workspace_path);

        if !path.exists() {
            return Err(ToolError::NotFound(format!(
                "File not found: {}",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(&path)?;
        let count = content.matches(&args.old_string).count();

        if count == 0 {
            return Err(ToolError::NotFound(
                "old_string not found in file".to_string(),
            ));
        }

        if count > 1 && !args.replace_all {
            return Err(ToolError::InvalidArgument(format!(
                "old_string found {} times. Use replace_all=true or provide more context.",
                count
            )));
        }

        let new_content = if args.replace_all {
            content.replace(&args.old_string, &args.new_string)
        } else {
            content.replacen(&args.old_string, &args.new_string, 1)
        };

        std::fs::write(&path, &new_content)?;

        Ok(ToolResult::success(format!(
            "Successfully replaced {} occurrence(s) in {}",
            if args.replace_all { count } else { 1 },
            path.display()
        )))
    }
}

fn resolve_path(path: &str, workspace: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        PathBuf::from(workspace).join(path)
    }
}
