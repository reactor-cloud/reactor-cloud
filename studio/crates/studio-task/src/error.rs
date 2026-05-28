use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Task not found: {0}")]
    NotFound(String),

    #[error("Invalid phase transition: cannot move from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    #[error("Phase not ready: {0}")]
    PhaseNotReady(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}
