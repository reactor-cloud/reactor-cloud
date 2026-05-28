// Ported from 1jehuang/jcode (MIT) - jcode-plan/src/parser.rs
// Adapted for Reactor Studio.

use crate::{Plan, PlanStep, PlanStatus};

/// Parser for markdown-formatted plans
pub struct PlanParser;

impl PlanParser {
    /// Parse a markdown plan
    pub fn parse(content: &str) -> Result<Plan, String> {
        let lines: Vec<&str> = content.lines().collect();
        
        if lines.is_empty() {
            return Err("Empty plan content".to_string());
        }

        let mut title = String::new();
        let mut summary = String::new();
        let mut steps = Vec::new();
        let mut in_summary = false;
        let mut current_step: Option<PlanStep> = None;

        for line in lines {
            let trimmed = line.trim();

            // Parse title (first H1)
            if trimmed.starts_with("# ") && title.is_empty() {
                title = trimmed.strip_prefix("# ").unwrap_or("").to_string();
                continue;
            }

            // Parse summary section
            if trimmed == "## Summary" || trimmed == "## Overview" {
                in_summary = true;
                continue;
            }

            // End summary on new H2
            if trimmed.starts_with("## ") && in_summary {
                in_summary = false;
            }

            if in_summary && !trimmed.is_empty() && !trimmed.starts_with('#') {
                if !summary.is_empty() {
                    summary.push(' ');
                }
                summary.push_str(trimmed);
                continue;
            }

            // Parse steps (numbered list or checkboxes)
            if let Some(step) = Self::parse_step_line(trimmed) {
                if let Some(prev) = current_step.take() {
                    steps.push(prev);
                }
                current_step = Some(step);
                continue;
            }

            // Parse substeps (indented list items)
            if let Some(substep) = Self::parse_substep_line(line) {
                if let Some(ref mut step) = current_step {
                    step.substeps.push(substep);
                }
            }
        }

        // Push last step
        if let Some(step) = current_step {
            steps.push(step);
        }

        if title.is_empty() {
            return Err("Plan must have a title".to_string());
        }

        Ok(Plan {
            title,
            summary,
            steps,
            metadata: Default::default(),
        })
    }

    fn parse_step_line(line: &str) -> Option<PlanStep> {
        let trimmed = line.trim();

        // Checkbox format: - [x] Step or - [ ] Step
        if let Some(rest) = trimmed.strip_prefix("- [") {
            let (status, title) = if rest.starts_with("x]") || rest.starts_with("X]") {
                (PlanStatus::Completed, rest.get(3..)?.trim())
            } else if rest.starts_with(" ]") {
                (PlanStatus::Pending, rest.get(3..)?.trim())
            } else if rest.starts_with("-]") {
                (PlanStatus::Skipped, rest.get(3..)?.trim())
            } else {
                return None;
            };

            let id = Self::generate_step_id(title);
            return Some(PlanStep {
                id,
                title: title.to_string(),
                description: String::new(),
                status,
                substeps: Vec::new(),
            });
        }

        // Numbered format: 1. Step
        if let Some(pos) = trimmed.find(". ") {
            let num = &trimmed[..pos];
            if num.chars().all(|c| c.is_ascii_digit()) {
                let title = &trimmed[pos + 2..];
                let id = Self::generate_step_id(title);
                return Some(PlanStep::new(id, title));
            }
        }

        None
    }

    fn parse_substep_line(line: &str) -> Option<PlanStep> {
        // Must be indented
        if !line.starts_with("  ") && !line.starts_with('\t') {
            return None;
        }

        Self::parse_step_line(line.trim())
    }

    fn generate_step_id(title: &str) -> String {
        title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .take(5)
            .collect::<Vec<_>>()
            .join("-")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_plan() {
        let content = r#"# My Plan

## Summary
This is a test plan.

## Steps
1. First step
2. Second step
"#;
        let plan = PlanParser::parse(content).unwrap();
        assert_eq!(plan.title, "My Plan");
        assert_eq!(plan.steps.len(), 2);
    }

    #[test]
    fn test_parse_checkbox_plan() {
        let content = r#"# Task Plan

- [x] Completed step
- [ ] Pending step
- [-] Skipped step
"#;
        let plan = PlanParser::parse(content).unwrap();
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].status, PlanStatus::Completed);
        assert_eq!(plan.steps[1].status, PlanStatus::Pending);
        assert_eq!(plan.steps[2].status, PlanStatus::Skipped);
    }
}
