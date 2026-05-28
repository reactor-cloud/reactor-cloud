use crate::error::{EvalError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tokio::fs;
use tracing::{debug, info};

/// The kind of scorer and its configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScorerKind {
    /// Check if a file exists
    FileExists {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<String>,
    },
    /// Check if a regex matches in a file
    RegexIn {
        file: String,
        pattern: String,
    },
    /// Check command exit code
    CommandExit {
        cmd: String,
        #[serde(default)]
        exit_code: i32,
    },
    /// Check git diff matches a pattern
    GitDiffMatches {
        pattern: String,
    },
    /// Run cargo check
    CargoCheck,
    /// Logical AND of multiple scorers
    All {
        scorers: Vec<ScorerKind>,
    },
    /// Logical OR of multiple scorers
    Any {
        scorers: Vec<ScorerKind>,
    },
    /// LLM judge (for L4+ tests)
    LlmJudge {
        rubric: String,
        #[serde(default = "default_llm_threshold")]
        threshold: f64,
    },
    /// Check function signature exists (structural)
    SignatureExists {
        file: String,
        signature: String,
    },
    /// Check for no unused imports (structural)
    NoUnusedImports {
        file: String,
    },
    /// Check type is exported (structural)
    TypeExported {
        file: String,
        type_name: String,
    },
}

fn default_llm_threshold() -> f64 {
    0.7
}

/// Result of a scorer evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreResult {
    pub passed: bool,
    pub scorer_kind: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ScoreResult {
    pub fn pass(kind: &str, message: impl Into<String>) -> Self {
        Self {
            passed: true,
            scorer_kind: kind.to_string(),
            message: message.into(),
            details: None,
        }
    }

    pub fn fail(kind: &str, message: impl Into<String>) -> Self {
        Self {
            passed: false,
            scorer_kind: kind.to_string(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// A scorer that evaluates test outcomes
pub struct Scorer;

impl Scorer {
    /// Evaluate a scorer against a worktree
    pub fn evaluate<'a>(kind: &'a ScorerKind, worktree_path: &'a Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ScoreResult>> + Send + 'a>> {
        Box::pin(async move {
            match kind {
                ScorerKind::FileExists { path, contains } => {
                    Self::eval_file_exists(worktree_path, path, contains.as_deref()).await
                }
                ScorerKind::RegexIn { file, pattern } => {
                    Self::eval_regex_in(worktree_path, file, pattern).await
                }
                ScorerKind::CommandExit { cmd, exit_code } => {
                    Self::eval_command_exit(worktree_path, cmd, *exit_code).await
                }
                ScorerKind::GitDiffMatches { pattern } => {
                    Self::eval_git_diff_matches(worktree_path, pattern).await
                }
                ScorerKind::CargoCheck => Self::eval_cargo_check(worktree_path).await,
                ScorerKind::All { scorers } => Self::eval_all(scorers, worktree_path).await,
                ScorerKind::Any { scorers } => Self::eval_any(scorers, worktree_path).await,
                ScorerKind::LlmJudge { rubric, threshold } => {
                    Self::eval_llm_judge(worktree_path, rubric, *threshold).await
                }
                ScorerKind::SignatureExists { file, signature } => {
                    Self::eval_signature_exists(worktree_path, file, signature).await
                }
                ScorerKind::NoUnusedImports { file } => {
                    Self::eval_no_unused_imports(worktree_path, file).await
                }
                ScorerKind::TypeExported { file, type_name } => {
                    Self::eval_type_exported(worktree_path, file, type_name).await
                }
            }
        })
    }

    async fn eval_file_exists(
        worktree: &Path,
        path: &str,
        contains: Option<&str>,
    ) -> Result<ScoreResult> {
        let full_path = worktree.join(path);

        if !full_path.exists() {
            return Ok(ScoreResult::fail(
                "file_exists",
                format!("File does not exist: {}", path),
            ));
        }

        if let Some(needle) = contains {
            let content = fs::read_to_string(&full_path).await?;
            if !content.contains(needle) {
                return Ok(ScoreResult::fail(
                    "file_exists",
                    format!("File {} does not contain: {}", path, needle),
                )
                .with_details(format!("File content:\n{}", content)));
            }
        }

        Ok(ScoreResult::pass(
            "file_exists",
            format!("File exists: {}", path),
        ))
    }

    async fn eval_regex_in(worktree: &Path, file: &str, pattern: &str) -> Result<ScoreResult> {
        let full_path = worktree.join(file);

        if !full_path.exists() {
            return Ok(ScoreResult::fail(
                "regex_in",
                format!("File does not exist: {}", file),
            ));
        }

        let content = fs::read_to_string(&full_path).await?;
        let regex = Regex::new(pattern).map_err(|e| EvalError::Scorer(e.to_string()))?;

        if regex.is_match(&content) {
            Ok(ScoreResult::pass(
                "regex_in",
                format!("Pattern '{}' found in {}", pattern, file),
            ))
        } else {
            Ok(ScoreResult::fail(
                "regex_in",
                format!("Pattern '{}' not found in {}", pattern, file),
            )
            .with_details(format!("File content:\n{}", content)))
        }
    }

    async fn eval_command_exit(
        worktree: &Path,
        cmd: &str,
        expected_exit_code: i32,
    ) -> Result<ScoreResult> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(worktree)
            .output()?;

        let actual_code = output.status.code().unwrap_or(-1);

        if actual_code == expected_exit_code {
            Ok(ScoreResult::pass(
                "command_exit",
                format!("Command '{}' exited with {}", cmd, actual_code),
            ))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(ScoreResult::fail(
                "command_exit",
                format!(
                    "Command '{}' exited with {} (expected {})",
                    cmd, actual_code, expected_exit_code
                ),
            )
            .with_details(format!("stderr:\n{}", stderr)))
        }
    }

    async fn eval_git_diff_matches(worktree: &Path, pattern: &str) -> Result<ScoreResult> {
        let output = Command::new("git")
            .args(["diff", "--staged", "--name-only"])
            .current_dir(worktree)
            .output()?;

        let diff_output = String::from_utf8_lossy(&output.stdout);
        let regex = Regex::new(pattern).map_err(|e| EvalError::Scorer(e.to_string()))?;

        if regex.is_match(&diff_output) {
            Ok(ScoreResult::pass(
                "git_diff_matches",
                format!("Git diff matches pattern '{}'", pattern),
            ))
        } else {
            Ok(ScoreResult::fail(
                "git_diff_matches",
                format!("Git diff does not match pattern '{}'", pattern),
            )
            .with_details(format!("Diff output:\n{}", diff_output)))
        }
    }

    async fn eval_cargo_check(worktree: &Path) -> Result<ScoreResult> {
        let output = Command::new("cargo")
            .arg("check")
            .current_dir(worktree)
            .output()?;

        if output.status.success() {
            Ok(ScoreResult::pass("cargo_check", "cargo check passed"))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(ScoreResult::fail("cargo_check", "cargo check failed")
                .with_details(format!("stderr:\n{}", stderr)))
        }
    }

    async fn eval_all(scorers: &[ScorerKind], worktree: &Path) -> Result<ScoreResult> {
        let mut all_passed = true;
        let mut messages = Vec::new();

        for scorer in scorers {
            let result = Self::evaluate(scorer, worktree).await?;
            if !result.passed {
                all_passed = false;
            }
            messages.push(format!(
                "  - {}: {} {}",
                result.scorer_kind,
                if result.passed { "PASS" } else { "FAIL" },
                result.message
            ));
        }

        if all_passed {
            Ok(ScoreResult::pass("all", "All scorers passed")
                .with_details(messages.join("\n")))
        } else {
            Ok(ScoreResult::fail("all", "Some scorers failed")
                .with_details(messages.join("\n")))
        }
    }

    async fn eval_any(scorers: &[ScorerKind], worktree: &Path) -> Result<ScoreResult> {
        let mut any_passed = false;
        let mut messages = Vec::new();

        for scorer in scorers {
            let result = Self::evaluate(scorer, worktree).await?;
            if result.passed {
                any_passed = true;
            }
            messages.push(format!(
                "  - {}: {} {}",
                result.scorer_kind,
                if result.passed { "PASS" } else { "FAIL" },
                result.message
            ));
        }

        if any_passed {
            Ok(ScoreResult::pass("any", "At least one scorer passed")
                .with_details(messages.join("\n")))
        } else {
            Ok(ScoreResult::fail("any", "No scorers passed")
                .with_details(messages.join("\n")))
        }
    }

    async fn eval_signature_exists(
        worktree: &Path,
        file: &str,
        signature: &str,
    ) -> Result<ScoreResult> {
        let full_path = worktree.join(file);
        
        if !full_path.exists() {
            return Ok(ScoreResult::fail(
                "signature_exists",
                format!("File does not exist: {}", file),
            ));
        }

        let content = fs::read_to_string(&full_path).await?;
        
        // Basic structural check: look for the signature pattern
        // In a full implementation, this would use an AST parser or LSP
        let pattern = format!(r"(?m)^\s*(pub\s+)?(fn|struct|enum|trait|type|impl)\s+{}", regex::escape(signature));
        let regex = Regex::new(&pattern).map_err(|e| EvalError::Scorer(e.to_string()))?;

        if regex.is_match(&content) {
            Ok(ScoreResult::pass(
                "signature_exists",
                format!("Signature '{}' found in {}", signature, file),
            ))
        } else {
            Ok(ScoreResult::fail(
                "signature_exists",
                format!("Signature '{}' not found in {}", signature, file),
            )
            .with_details(format!("Searched for pattern: {}", pattern)))
        }
    }

    async fn eval_no_unused_imports(worktree: &Path, file: &str) -> Result<ScoreResult> {
        let full_path = worktree.join(file);
        
        if !full_path.exists() {
            return Ok(ScoreResult::fail(
                "no_unused_imports",
                format!("File does not exist: {}", file),
            ));
        }

        // Run rustc or cargo check with warnings to detect unused imports
        // For Rust files, use cargo check with warnings
        if file.ends_with(".rs") {
            let output = Command::new("cargo")
                .args(["check", "--message-format=short"])
                .current_dir(worktree)
                .output()?;

            let stderr = String::from_utf8_lossy(&output.stderr);
            
            // Look for unused import warnings related to this file
            let file_name = std::path::Path::new(file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file);
            
            let has_unused_imports = stderr.contains("unused import") 
                && stderr.contains(file_name);

            if has_unused_imports {
                return Ok(ScoreResult::fail(
                    "no_unused_imports",
                    format!("Unused imports found in {}", file),
                )
                .with_details(stderr.to_string()));
            }
        }

        Ok(ScoreResult::pass(
            "no_unused_imports",
            format!("No unused imports in {}", file),
        ))
    }

    async fn eval_type_exported(
        worktree: &Path,
        file: &str,
        type_name: &str,
    ) -> Result<ScoreResult> {
        let full_path = worktree.join(file);
        
        if !full_path.exists() {
            return Ok(ScoreResult::fail(
                "type_exported",
                format!("File does not exist: {}", file),
            ));
        }

        let content = fs::read_to_string(&full_path).await?;
        
        // Look for pub struct/enum/type with the given name
        let pattern = format!(
            r"(?m)^\s*pub\s+(struct|enum|type)\s+{}\b",
            regex::escape(type_name)
        );
        let regex = Regex::new(&pattern).map_err(|e| EvalError::Scorer(e.to_string()))?;

        if regex.is_match(&content) {
            Ok(ScoreResult::pass(
                "type_exported",
                format!("Type '{}' is exported from {}", type_name, file),
            ))
        } else {
            // Check if type exists but is not pub
            let private_pattern = format!(
                r"(?m)^\s*(struct|enum|type)\s+{}\b",
                regex::escape(type_name)
            );
            let private_regex = Regex::new(&private_pattern).map_err(|e| EvalError::Scorer(e.to_string()))?;

            if private_regex.is_match(&content) {
                Ok(ScoreResult::fail(
                    "type_exported",
                    format!("Type '{}' exists but is not exported (not pub)", type_name),
                ))
            } else {
                Ok(ScoreResult::fail(
                    "type_exported",
                    format!("Type '{}' not found in {}", type_name, file),
                ))
            }
        }
    }

    async fn eval_llm_judge(
        worktree: &Path,
        rubric: &str,
        threshold: f64,
    ) -> Result<ScoreResult> {
        // LLM judge implementation would call the LLM provider
        // For now, return a stub that can be implemented with studio-providers
        
        // TODO: Integrate with studio-providers to call an LLM
        // The implementation should:
        // 1. Read the relevant files from the worktree
        // 2. Construct a prompt with the rubric
        // 3. Call the LLM and parse the response
        // 4. Return pass/fail based on threshold
        
        Ok(ScoreResult::fail(
            "llm_judge",
            format!("LLM judge scorer requires provider integration (rubric: {:.50}...)", rubric),
        )
        .with_details(format!("Threshold: {}, rubric provided but LLM call not yet implemented", threshold)))
    }
}
