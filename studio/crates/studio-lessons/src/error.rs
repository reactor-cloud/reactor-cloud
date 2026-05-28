use thiserror::Error;

#[derive(Error, Debug)]
pub enum LessonError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Lesson not found: {0}")]
    NotFound(String),

    #[error("Invalid tier transition: {from:?} -> {to:?}")]
    InvalidTierTransition { from: super::Tier, to: super::Tier },

    #[error("Ledger corrupted at line {line}: {message}")]
    LedgerCorrupted { line: usize, message: String },

    #[error("Store path not initialized")]
    StoreNotInitialized,
}

pub type Result<T> = std::result::Result<T, LessonError>;
