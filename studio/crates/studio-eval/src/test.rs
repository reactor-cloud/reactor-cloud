use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique identifier for a test
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestId(pub String);

impl std::fmt::Display for TestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Test level (L0-L7)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TestLevel {
    L0,
    L1,
    L2,
    L3,
    L4,
    L5,
    L6,
    L7,
}

impl TestLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestLevel::L0 => "L0",
            TestLevel::L1 => "L1",
            TestLevel::L2 => "L2",
            TestLevel::L3 => "L3",
            TestLevel::L4 => "L4",
            TestLevel::L5 => "L5",
            TestLevel::L6 => "L6",
            TestLevel::L7 => "L7",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "L0" => Some(TestLevel::L0),
            "L1" => Some(TestLevel::L1),
            "L2" => Some(TestLevel::L2),
            "L3" => Some(TestLevel::L3),
            "L4" => Some(TestLevel::L4),
            "L5" => Some(TestLevel::L5),
            "L6" => Some(TestLevel::L6),
            "L7" => Some(TestLevel::L7),
            _ => None,
        }
    }

    pub fn is_smoke(&self) -> bool {
        matches!(self, TestLevel::L0 | TestLevel::L1 | TestLevel::L2)
    }

    pub fn is_standard(&self) -> bool {
        matches!(self, TestLevel::L3 | TestLevel::L4)
    }

    pub fn is_full(&self) -> bool {
        matches!(self, TestLevel::L5 | TestLevel::L6 | TestLevel::L7)
    }
}

/// Budget constraints for a test run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    #[serde(default = "default_max_tool_calls")]
    pub max_tool_calls: u64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u64,
    #[serde(default = "default_max_wallclock_secs")]
    pub max_wallclock_secs: u64,
}

fn default_max_tool_calls() -> u64 {
    30
}
fn default_max_tokens() -> u64 {
    50000
}
fn default_max_wallclock_secs() -> u64 {
    300
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_tool_calls: default_max_tool_calls(),
            max_tokens: default_max_tokens(),
            max_wallclock_secs: default_max_wallclock_secs(),
        }
    }
}

/// Model pinning for reproducible runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPin {
    pub provider: String,
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_temperature() -> f32 {
    0.2
}

impl Default for ModelPin {
    fn default() -> Self {
        Self {
            provider: "openrouter".to_string(),
            model: "anthropic/claude-sonnet-4".to_string(),
            temperature: default_temperature(),
        }
    }
}

/// Fixture specification for a test
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fixture {
    /// Empty directory
    Empty,
    /// Git repository at a specific ref
    Git { repo: String, ref_: String },
    /// Tarball to extract
    Tarball { path: PathBuf },
    /// Copy from a template directory
    Template { path: PathBuf },
}

impl Default for Fixture {
    fn default() -> Self {
        Fixture::Empty
    }
}

/// A test definition loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Test {
    pub id: TestId,
    pub level: TestLevel,
    #[serde(default)]
    pub domain: Vec<String>,
    #[serde(default)]
    pub phases: Vec<String>,
    #[serde(default)]
    pub fixture: Fixture,
    pub instruction: String,
    pub success: Vec<crate::scorer::ScorerKind>,
    #[serde(default)]
    pub budget: Budget,
    #[serde(default = "default_runs_required")]
    pub runs_required: u32,
    #[serde(default = "default_pass_threshold")]
    pub pass_threshold: f64,
    #[serde(default)]
    pub model_pin: Option<ModelPin>,
}

fn default_runs_required() -> u32 {
    1
}
fn default_pass_threshold() -> f64 {
    1.0
}

impl Test {
    pub fn is_deterministic(&self) -> bool {
        self.runs_required == 1 && self.pass_threshold == 1.0
    }
}
