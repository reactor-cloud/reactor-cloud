// Ported from 1jehuang/jcode (MIT) - jcode-plan/src/types.rs
// Adapted for Reactor Studio.

use serde::{Deserialize, Serialize};

/// Status of a plan step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Pending,
    InProgress,
    Completed,
    Skipped,
    Failed,
}

impl Default for PlanStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// A single step in a plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: PlanStatus,
    #[serde(default)]
    pub substeps: Vec<PlanStep>,
}

impl PlanStep {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: String::new(),
            status: PlanStatus::Pending,
            substeps: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_substeps(mut self, substeps: Vec<PlanStep>) -> Self {
        self.substeps = substeps;
        self
    }
}

/// A complete plan for a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub title: String,
    #[serde(default)]
    pub summary: String,
    pub steps: Vec<PlanStep>,
    #[serde(default)]
    pub metadata: PlanMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanMetadata {
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub estimated_effort: Option<String>,
}

impl Plan {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            summary: String::new(),
            steps: Vec::new(),
            metadata: PlanMetadata::default(),
        }
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn with_steps(mut self, steps: Vec<PlanStep>) -> Self {
        self.steps = steps;
        self
    }

    /// Count total steps including substeps
    pub fn total_steps(&self) -> usize {
        fn count_steps(steps: &[PlanStep]) -> usize {
            steps
                .iter()
                .map(|s| 1 + count_steps(&s.substeps))
                .sum()
        }
        count_steps(&self.steps)
    }

    /// Count completed steps
    pub fn completed_steps(&self) -> usize {
        fn count_completed(steps: &[PlanStep]) -> usize {
            steps
                .iter()
                .map(|s| {
                    let this = if s.status == PlanStatus::Completed { 1 } else { 0 };
                    this + count_completed(&s.substeps)
                })
                .sum()
        }
        count_completed(&self.steps)
    }

    /// Get progress as a percentage
    pub fn progress(&self) -> f32 {
        let total = self.total_steps();
        if total == 0 {
            return 0.0;
        }
        (self.completed_steps() as f32 / total as f32) * 100.0
    }
}
