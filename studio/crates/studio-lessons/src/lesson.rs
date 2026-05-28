use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Unique identifier for a lesson
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LessonId(pub String);

impl LessonId {
    pub fn new() -> Self {
        Self(format!("L{}", Uuid::new_v4().simple()))
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for LessonId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LessonId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The tier of a lesson in the promotion ladder
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Active hypothesis - scratch buffer in active task
    T0,
    /// Staged - in .foundry/lessons/staged/
    T1,
    /// Validated - in .foundry/lessons/validated/
    T2,
    /// Established - in .foundry/lessons/established/ or global
    T3,
    /// Adopted - merged into conventions, prompts, skills, or code
    T4,
}

impl Tier {
    pub fn weight(&self) -> f64 {
        match self {
            Tier::T0 => 0.1,
            Tier::T1 => 0.3,
            Tier::T2 => 0.5,
            Tier::T3 => 0.8,
            Tier::T4 => 1.0,
        }
    }

    pub fn storage_dir(&self) -> &'static str {
        match self {
            Tier::T0 => "scratch",
            Tier::T1 => "staged",
            Tier::T2 => "validated",
            Tier::T3 => "established",
            Tier::T4 => "adopted",
        }
    }
}

/// The scope of a lesson
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Project-specific lesson
    Project,
    /// Candidate for global promotion
    GlobalCandidate,
    /// Globally applicable lesson
    Global,
}

/// The origin of a lesson
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// Extracted from a postmortem analysis
    Postmortem,
    /// Generated from synthetic project runs
    Synthetic,
    /// Received from upstream (Foundry)
    Upstream,
    /// Manually created
    Manual,
}

/// The kind of lesson content
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LessonKind {
    /// A snippet to add to a prompt or conventions file
    PromptDelta {
        target: PromptTarget,
        snippet: String,
    },
    /// A heuristic rule: when X, prefer Y, avoid Z
    Heuristic {
        when: String,
        prefer: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        avoid: Option<String>,
    },
    /// A skill bundle manifest
    SkillBundle {
        manifest_path: PathBuf,
    },
    /// A proposal for a new tool (T4-only)
    ToolProposal {
        name: String,
        description: String,
        spec: String,
    },
    /// A pattern to avoid
    AntiPattern {
        pattern: String,
        reason: String,
    },
}

/// Target for a prompt delta
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptTarget {
    SharedConventions,
    AgentPrompt(String),
    PhasePrompt(String),
}

/// Kind of trigger for lesson retrieval
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    /// Triggered by a tool error signature
    ToolErrorSignature,
    /// Triggered at phase start
    PhaseStart,
    /// Triggered by regex match in message
    RegexInMessage,
    /// Triggered by domain tag
    DomainTag,
}

/// A trigger condition for lesson retrieval
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    pub kind: TriggerKind,
    pub value: String,
}

/// Version constraints for lesson validity
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Constraints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_reactor_cli: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_reactor_cli: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
}

/// A lesson - the core artifact of the learning system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    pub id: LessonId,
    pub kind: LessonKind,
    pub tier: Tier,
    pub scope: Scope,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub phases: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub valid_for: Constraints,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    pub origin: Origin,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    #[serde(default)]
    pub citations: u64,
    #[serde(default)]
    pub successes: u64,
    #[serde(default)]
    pub failures: u64,
}

impl Lesson {
    pub fn new(kind: LessonKind, title: String, body: String, origin: Origin) -> Self {
        let now = Utc::now();
        Self {
            id: LessonId::new(),
            kind,
            tier: Tier::T0,
            scope: Scope::Project,
            title,
            body,
            tags: Vec::new(),
            phases: Vec::new(),
            triggers: Vec::new(),
            valid_for: Constraints::default(),
            embedding: None,
            origin,
            created: now,
            updated: now,
            citations: 0,
            successes: 0,
            failures: 0,
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.citations == 0 {
            0.0
        } else {
            self.successes as f64 / self.citations as f64
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_phases(mut self, phases: Vec<String>) -> Self {
        self.phases = phases;
        self
    }

    pub fn with_triggers(mut self, triggers: Vec<Trigger>) -> Self {
        self.triggers = triggers;
        self
    }

    pub fn with_scope(mut self, scope: Scope) -> Self {
        self.scope = scope;
        self
    }
}
