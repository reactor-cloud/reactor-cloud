use crate::error::{PostmortemError, Result};
use crate::scope_classifier::ScopeClassifier;
use serde::{Deserialize, Serialize};
use studio_lessons::{Lesson, LessonKind, Origin, Scope};
use tracing::{debug, info, warn};

/// A trace of a failed test run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureTrace {
    pub test_id: String,
    pub phase: String,
    pub failure_signature: String,
    pub tool_calls: Vec<ToolCallRecord>,
    pub error_messages: Vec<String>,
    pub recovery_attempts: Vec<RecoveryAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub name: String,
    pub args: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    pub strategy: String,
    pub succeeded: bool,
    pub steps: Vec<String>,
}

/// A candidate lesson extracted from a postmortem analysis
#[derive(Debug, Clone)]
pub struct LessonCandidate {
    pub lesson: Lesson,
    pub confidence: f64,
    pub extraction_reason: String,
}

/// The postmortem agent that analyzes failure traces and extracts lessons
pub struct Postmortem {
    scope_classifier: ScopeClassifier,
}

impl Postmortem {
    pub fn new() -> Self {
        Self {
            scope_classifier: ScopeClassifier::new(),
        }
    }

    /// Analyze a failure trace and extract lesson candidates
    pub async fn analyze(&self, trace: &FailureTrace) -> Result<Vec<LessonCandidate>> {
        info!("Analyzing failure trace for test {}", trace.test_id);

        let mut candidates = Vec::new();

        // Extract lessons from different patterns in the trace
        candidates.extend(self.extract_from_error_patterns(trace).await?);
        candidates.extend(self.extract_from_recovery_patterns(trace).await?);
        candidates.extend(self.extract_from_tool_patterns(trace).await?);

        // Classify scope for each candidate
        for candidate in &mut candidates {
            let scope = self.scope_classifier.classify(&candidate.lesson).await?;
            candidate.lesson.scope = scope;
        }

        // Sort by confidence
        candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        info!(
            "Extracted {} lesson candidates from trace",
            candidates.len()
        );

        Ok(candidates)
    }

    async fn extract_from_error_patterns(&self, trace: &FailureTrace) -> Result<Vec<LessonCandidate>> {
        let mut candidates = Vec::new();

        // Look for common error patterns
        for error in &trace.error_messages {
            if let Some(candidate) = self.match_error_pattern(error, trace) {
                candidates.push(candidate);
            }
        }

        Ok(candidates)
    }

    fn match_error_pattern(&self, error: &str, trace: &FailureTrace) -> Option<LessonCandidate> {
        // Pattern: Module not found errors
        if error.contains("Cannot find module") || error.contains("Module not found") {
            let lesson = Lesson::new(
                LessonKind::Heuristic {
                    when: "module import error".to_string(),
                    prefer: "Check import paths and ensure dependencies are installed".to_string(),
                    avoid: None,
                },
                format!("Handle module import error: {}", &error[..error.len().min(50)]),
                format!(
                    "When encountering module import errors, first verify the import path is correct, \
                    then ensure the dependency is listed in package.json and installed."
                ),
                Origin::Postmortem,
            )
            .with_tags(vec!["import".to_string(), "module".to_string(), "dependency".to_string()]);

            return Some(LessonCandidate {
                lesson,
                confidence: 0.7,
                extraction_reason: "Matched module not found error pattern".to_string(),
            });
        }

        // Pattern: Type errors
        if error.contains("Type error") || error.contains("TS2") {
            let lesson = Lesson::new(
                LessonKind::Heuristic {
                    when: "TypeScript type error".to_string(),
                    prefer: "Check type definitions and ensure proper typing".to_string(),
                    avoid: Some("Using 'any' type without justification".to_string()),
                },
                format!("Handle TypeScript error: {}", &error[..error.len().min(50)]),
                format!(
                    "TypeScript errors often indicate a mismatch between expected and actual types. \
                    Review the type definitions and ensure proper typing throughout the call chain."
                ),
                Origin::Postmortem,
            )
            .with_tags(vec!["typescript".to_string(), "types".to_string()]);

            return Some(LessonCandidate {
                lesson,
                confidence: 0.6,
                extraction_reason: "Matched TypeScript error pattern".to_string(),
            });
        }

        // Pattern: Environment variable issues
        if error.contains("env") && (error.contains("undefined") || error.contains("not set")) {
            let lesson = Lesson::new(
                LessonKind::Heuristic {
                    when: "environment variable is undefined".to_string(),
                    prefer: "Define env var in .env and validate at startup".to_string(),
                    avoid: Some("Accessing process.env without validation".to_string()),
                },
                "Always validate environment variables at startup".to_string(),
                format!(
                    "Environment variables should be validated at application startup. \
                    Add them to .env.example for documentation and use a validation library \
                    or startup check to fail fast if required vars are missing."
                ),
                Origin::Postmortem,
            )
            .with_tags(vec!["env".to_string(), "configuration".to_string()]);

            return Some(LessonCandidate {
                lesson,
                confidence: 0.8,
                extraction_reason: "Matched environment variable error pattern".to_string(),
            });
        }

        None
    }

    async fn extract_from_recovery_patterns(&self, trace: &FailureTrace) -> Result<Vec<LessonCandidate>> {
        let mut candidates = Vec::new();

        // Look for successful recovery patterns that could be generalized
        for recovery in &trace.recovery_attempts {
            if recovery.succeeded {
                let lesson = Lesson::new(
                    LessonKind::Heuristic {
                        when: format!("encountering similar failure in {} phase", trace.phase),
                        prefer: format!("Apply recovery strategy: {}", recovery.strategy),
                        avoid: None,
                    },
                    format!("Recovery pattern: {}", recovery.strategy),
                    format!(
                        "When encountering this type of failure, the following recovery steps worked:\n{}",
                        recovery.steps.iter().enumerate()
                            .map(|(i, s)| format!("{}. {}", i + 1, s))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    Origin::Postmortem,
                )
                .with_phases(vec![trace.phase.clone()]);

                candidates.push(LessonCandidate {
                    lesson,
                    confidence: 0.75,
                    extraction_reason: format!("Successful recovery with strategy: {}", recovery.strategy),
                });
            }
        }

        Ok(candidates)
    }

    async fn extract_from_tool_patterns(&self, trace: &FailureTrace) -> Result<Vec<LessonCandidate>> {
        let mut candidates = Vec::new();

        // Look for repeated tool call patterns that might indicate a learning opportunity
        let mut tool_errors: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

        for call in &trace.tool_calls {
            if let Some(error) = &call.error {
                tool_errors
                    .entry(call.name.clone())
                    .or_default()
                    .push(error.clone());
            }
        }

        for (tool_name, errors) in tool_errors {
            if errors.len() >= 2 {
                // Repeated errors with the same tool
                let lesson = Lesson::new(
                    LessonKind::AntiPattern {
                        pattern: format!("Repeated {} errors", tool_name),
                        reason: format!(
                            "The {} tool failed multiple times with similar errors. \
                            Consider checking preconditions before calling.",
                            tool_name
                        ),
                    },
                    format!("Avoid repeated {} failures", tool_name),
                    format!(
                        "The {} tool showed repeated failures:\n- {}\n\n\
                        Before calling this tool, verify the preconditions are met.",
                        tool_name,
                        errors.join("\n- ")
                    ),
                    Origin::Postmortem,
                )
                .with_tags(vec!["tool".to_string(), tool_name.clone()]);

                candidates.push(LessonCandidate {
                    lesson,
                    confidence: 0.65,
                    extraction_reason: format!("Repeated errors with {} tool", tool_name),
                });
            }
        }

        Ok(candidates)
    }
}

impl Default for Postmortem {
    fn default() -> Self {
        Self::new()
    }
}
