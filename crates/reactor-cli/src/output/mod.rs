//! Output formatting for CLI commands.
//!
//! Supports human-readable (default for TTY) and JSON (default for non-TTY) output.

pub mod human;
pub mod json;

use crate::error::{CliError, CliResult};
use serde::Serialize;

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Human-readable output with colors and tables.
    #[default]
    Human,
    /// JSON output for scripting and AI agents.
    Json,
}

impl OutputFormat {
    /// Resolve the output format based on explicit selection and TTY detection.
    ///
    /// - If `explicit` is Some, use that.
    /// - Otherwise, use Human for TTY, Json for non-TTY.
    pub fn resolve(explicit: Option<OutputFormat>) -> Self {
        match explicit {
            Some(fmt) => fmt,
            None => {
                if console::Term::stdout().is_term() {
                    OutputFormat::Human
                } else {
                    OutputFormat::Json
                }
            }
        }
    }

    /// Whether this is JSON format.
    pub fn is_json(&self) -> bool {
        matches!(self, OutputFormat::Json)
    }

    /// Whether this is human format.
    pub fn is_human(&self) -> bool {
        matches!(self, OutputFormat::Human)
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" | "text" => Ok(OutputFormat::Human),
            "json" => Ok(OutputFormat::Json),
            _ => Err(format!("invalid output format: {}", s)),
        }
    }
}

/// Output writer that handles both success and error cases.
pub struct Output {
    format: OutputFormat,
}

impl Output {
    /// Create a new output writer with the given format.
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Get the output format.
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// Write a successful result.
    pub fn success<T: Serialize + std::fmt::Debug>(&self, data: &T) -> CliResult<()> {
        match self.format {
            OutputFormat::Human => {
                // Human output is handled by specific commands
                // This is a fallback that prints debug output
                println!("{:#?}", data);
            }
            OutputFormat::Json => {
                json::write_success(data)?;
            }
        }
        Ok(())
    }

    /// Write a success message (no data).
    pub fn success_message(&self, message: &str) -> CliResult<()> {
        match self.format {
            OutputFormat::Human => {
                human::print_success(message);
            }
            OutputFormat::Json => {
                json::write_success(&serde_json::json!({ "message": message }))?;
            }
        }
        Ok(())
    }

    /// Write an error.
    pub fn error(&self, error: &CliError) {
        match self.format {
            OutputFormat::Human => {
                human::print_error(error);
            }
            OutputFormat::Json => {
                if let Err(e) = json::write_error(error) {
                    eprintln!("Failed to write JSON error: {}", e);
                    eprintln!("Original error: {}", error);
                }
            }
        }
    }

    /// Write a warning message.
    pub fn warning(&self, message: &str) {
        match self.format {
            OutputFormat::Human => {
                human::print_warning(message);
            }
            OutputFormat::Json => {
                // Warnings are typically not shown in JSON mode
                // unless they're part of the response data
            }
        }
    }

    /// Write an info message.
    pub fn info(&self, message: &str) {
        match self.format {
            OutputFormat::Human => {
                human::print_info(message);
            }
            OutputFormat::Json => {
                // Info messages are not shown in JSON mode
            }
        }
    }

    /// Print a table with headers and rows.
    pub fn table(&self, headers: &[&str], rows: Vec<Vec<String>>) -> CliResult<()> {
        match self.format {
            OutputFormat::Human => {
                human::print_table(headers, rows);
            }
            OutputFormat::Json => {
                // Convert table to array of objects
                let objects: Vec<serde_json::Value> = rows
                    .into_iter()
                    .map(|row| {
                        let mut obj = serde_json::Map::new();
                        for (i, cell) in row.into_iter().enumerate() {
                            let key = headers.get(i).map(|s| *s).unwrap_or("_");
                            obj.insert(key.to_string(), serde_json::Value::String(cell));
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect();
                json::write_success(&objects)?;
            }
        }
        Ok(())
    }
}

/// Check if stdout is a TTY.
pub fn is_tty() -> bool {
    console::Term::stdout().is_term()
}

/// Check if stdin is a TTY.
pub fn is_stdin_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}
