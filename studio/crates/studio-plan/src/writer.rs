// Ported from 1jehuang/jcode (MIT) - jcode-plan/src/writer.rs
// Adapted for Reactor Studio.

use crate::{Plan, PlanStep, PlanStatus};

/// Writer for markdown-formatted plans
pub struct PlanWriter;

impl PlanWriter {
    /// Render a plan to markdown
    pub fn render(plan: &Plan) -> String {
        let mut output = String::new();

        // Title
        output.push_str(&format!("# {}\n\n", plan.title));

        // Summary
        if !plan.summary.is_empty() {
            output.push_str("## Summary\n\n");
            output.push_str(&plan.summary);
            output.push_str("\n\n");
        }

        // Progress
        let progress = plan.progress();
        output.push_str(&format!(
            "**Progress:** {:.0}% ({}/{})\n\n",
            progress,
            plan.completed_steps(),
            plan.total_steps()
        ));

        // Steps
        output.push_str("## Steps\n\n");
        for step in &plan.steps {
            Self::render_step(&mut output, step, 0);
        }

        output
    }

    fn render_step(output: &mut String, step: &PlanStep, indent: usize) {
        let indent_str = "  ".repeat(indent);
        let checkbox = Self::status_to_checkbox(step.status);

        output.push_str(&format!(
            "{}- [{}] {}\n",
            indent_str, checkbox, step.title
        ));

        if !step.description.is_empty() {
            output.push_str(&format!(
                "{}  _{}_\n",
                indent_str, step.description
            ));
        }

        for substep in &step.substeps {
            Self::render_step(output, substep, indent + 1);
        }
    }

    fn status_to_checkbox(status: PlanStatus) -> char {
        match status {
            PlanStatus::Completed => 'x',
            PlanStatus::Pending => ' ',
            PlanStatus::InProgress => '~',
            PlanStatus::Skipped => '-',
            PlanStatus::Failed => '!',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plan() {
        let plan = Plan::new("Test Plan")
            .with_summary("A test")
            .with_steps(vec![
                PlanStep::new("1", "First step"),
                PlanStep {
                    id: "2".to_string(),
                    title: "Second step".to_string(),
                    description: String::new(),
                    status: PlanStatus::Completed,
                    substeps: vec![],
                },
            ]);

        let output = PlanWriter::render(&plan);
        assert!(output.contains("# Test Plan"));
        assert!(output.contains("- [ ] First step"));
        assert!(output.contains("- [x] Second step"));
    }
}
