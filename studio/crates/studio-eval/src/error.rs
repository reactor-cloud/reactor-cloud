use thiserror::Error;

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Test not found: {0}")]
    TestNotFound(String),

    #[error("Fixture not found: {0}")]
    FixtureNotFound(String),

    #[error("Worktree error: {0}")]
    Worktree(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Runner error: {0}")]
    Runner(String),

    #[error("Scorer error: {0}")]
    Scorer(String),

    #[error("Replay cassette miss: request hash {request_hash}")]
    CassetteMiss { request_hash: String },

    #[error("Replay cassette not found: {0}")]
    CassetteNotFound(String),

    #[error("Budget exceeded: {kind} ({used} > {limit})")]
    BudgetExceeded {
        kind: String,
        used: u64,
        limit: u64,
    },

    #[error("Lesson error: {0}")]
    Lesson(#[from] studio_lessons::LessonError),

    #[error("Command failed: {command} (exit code: {exit_code:?})")]
    CommandFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
    },
}

pub type Result<T> = std::result::Result<T, EvalError>;
