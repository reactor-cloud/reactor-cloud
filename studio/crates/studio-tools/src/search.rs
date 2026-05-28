// Ported from 1jehuang/jcode (MIT) - src/tool/{grep,glob}.rs
// Adapted for Reactor Studio.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

use crate::{Tool, ToolContext, ToolError, ToolResult};

/// Tool for searching file contents using ripgrep
pub struct GrepTool;

#[derive(Deserialize)]
struct GrepArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a regex pattern in files using ripgrep. Returns matching lines with file paths and line numbers."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (defaults to workspace root)"
                },
                "glob": {
                    "type": "string",
                    "description": "File pattern to filter (e.g., '*.ts', '*.rs')"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 50)"
                }
            }
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: GrepArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgument(e.to_string()))?;

        let search_path = args
            .path
            .map(|p| resolve_path(&p, &ctx.workspace_path))
            .unwrap_or_else(|| PathBuf::from(&ctx.workspace_path));

        let mut cmd = Command::new("rg");
        cmd.arg("--line-number")
            .arg("--no-heading")
            .arg("--color=never")
            .arg("-m")
            .arg(args.limit.to_string());

        if let Some(glob) = &args.glob {
            cmd.arg("-g").arg(glob);
        }

        cmd.arg(&args.pattern).arg(&search_path);

        let output = cmd.output()?;

        if output.status.success() || output.status.code() == Some(1) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.is_empty() {
                Ok(ToolResult::success("No matches found."))
            } else {
                Ok(ToolResult::success(stdout.to_string()))
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ToolError::ExecutionFailed(stderr.to_string()))
        }
    }
}

/// Tool for finding files using glob patterns
pub struct GlobTool;

#[derive(Deserialize)]
struct GlobArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Returns matching file paths."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g., '**/*.ts', 'src/**/*.rs')"
                },
                "path": {
                    "type": "string",
                    "description": "Base directory to search from (defaults to workspace root)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 50)"
                }
            }
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: GlobArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgument(e.to_string()))?;

        let base_path = args
            .path
            .as_ref()
            .map(|p| resolve_path(p, &ctx.workspace_path))
            .unwrap_or_else(|| PathBuf::from(&ctx.workspace_path));

        let pattern = if args.pattern.starts_with('/') || args.pattern.contains(':') {
            args.pattern.clone()
        } else {
            format!("{}/{}", base_path.display(), args.pattern)
        };

        let entries: Vec<_> = glob::glob(&pattern)
            .map_err(|e| ToolError::InvalidArgument(format!("Invalid glob pattern: {}", e)))?
            .filter_map(|e| e.ok())
            .take(args.limit)
            .collect();

        if entries.is_empty() {
            Ok(ToolResult::success("No files found matching the pattern."))
        } else {
            let output = entries
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            Ok(ToolResult::success(format!(
                "Found {} files:\n{}",
                entries.len(),
                output
            )))
        }
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
