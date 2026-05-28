use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Unique identifier for a task
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    pub fn new() -> Self {
        Self(format!("task_{}", uuid_simple()))
    }

    pub fn from_string(s: &str) -> Self {
        Self(s.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

/// Task phases in order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Alignment,
    Planning,
    Development,
    Testing,
    Uat,
    Deployment,
}

impl Phase {
    pub const ALL: [Phase; 6] = [
        Phase::Alignment,
        Phase::Planning,
        Phase::Development,
        Phase::Testing,
        Phase::Uat,
        Phase::Deployment,
    ];

    pub fn index(&self) -> usize {
        match self {
            Phase::Alignment => 0,
            Phase::Planning => 1,
            Phase::Development => 2,
            Phase::Testing => 3,
            Phase::Uat => 4,
            Phase::Deployment => 5,
        }
    }

    pub fn from_index(idx: usize) -> Option<Self> {
        Self::ALL.get(idx).copied()
    }

    pub fn next(&self) -> Option<Self> {
        Self::from_index(self.index() + 1)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Phase::Alignment => "Alignment",
            Phase::Planning => "Planning",
            Phase::Development => "Development",
            Phase::Testing => "Testing",
            Phase::Uat => "UAT",
            Phase::Deployment => "Deployment",
        }
    }

    pub fn agent_id(&self) -> &'static str {
        match self {
            Phase::Alignment => "planner",
            Phase::Planning => "planner",
            Phase::Development => "coder",
            Phase::Testing => "coder",
            Phase::Uat => "coder",
            Phase::Deployment => "coder",
        }
    }
}

/// Status of a task phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    Locked,
    Active,
    Completed,
    Skipped,
}

impl Default for PhaseStatus {
    fn default() -> Self {
        Self::Locked
    }
}

/// State of a single phase within a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseState {
    pub phase: Phase,
    pub status: PhaseStatus,
    pub conversation_id: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl PhaseState {
    pub fn new(phase: Phase) -> Self {
        Self {
            phase,
            status: if phase == Phase::Alignment {
                PhaseStatus::Active
            } else {
                PhaseStatus::Locked
            },
            conversation_id: None,
            started_at: None,
            completed_at: None,
        }
    }
}

/// Overall task state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Active,
    Completed,
    Abandoned,
}

impl Default for TaskState {
    fn default() -> Self {
        Self::Active
    }
}

/// Full task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub state: TaskState,
    pub current_phase: Phase,
    pub phases: Vec<PhaseState>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub artifacts: TaskArtifacts,
}

impl Task {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        let phases = Phase::ALL.iter().map(|&p| PhaseState::new(p)).collect();

        Self {
            id: TaskId::new(),
            title: title.into(),
            description: String::new(),
            state: TaskState::Active,
            current_phase: Phase::Alignment,
            phases,
            created_at: now,
            updated_at: now,
            artifacts: TaskArtifacts::default(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn current_phase_state(&self) -> &PhaseState {
        &self.phases[self.current_phase.index()]
    }

    pub fn current_phase_state_mut(&mut self) -> &mut PhaseState {
        let idx = self.current_phase.index();
        &mut self.phases[idx]
    }

    pub fn can_advance(&self) -> bool {
        self.current_phase.next().is_some()
            && self.current_phase_state().status == PhaseStatus::Active
    }

    pub fn is_completed(&self) -> bool {
        self.state == TaskState::Completed
    }
}

/// Artifacts produced during a task
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskArtifacts {
    #[serde(default)]
    pub plan_path: Option<String>,
    #[serde(default)]
    pub test_report_path: Option<String>,
    #[serde(default)]
    pub deployment_url: Option<String>,
}

/// Summary of a task for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub state: TaskState,
    pub current_phase: Phase,
    pub progress: f32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&Task> for TaskSummary {
    fn from(task: &Task) -> Self {
        let completed = task
            .phases
            .iter()
            .filter(|p| p.status == PhaseStatus::Completed)
            .count();
        let total = task.phases.len();
        let progress = (completed as f32 / total as f32) * 100.0;

        Self {
            id: task.id.as_str().to_string(),
            title: task.title.clone(),
            state: task.state,
            current_phase: task.current_phase,
            progress,
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
        }
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", now)
}
