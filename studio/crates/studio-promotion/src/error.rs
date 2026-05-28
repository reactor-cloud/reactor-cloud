use thiserror::Error;

#[derive(Error, Debug)]
pub enum PromotionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Lesson error: {0}")]
    Lesson(#[from] studio_lessons::LessonError),

    #[error("Invalid tier transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: studio_lessons::Tier,
        to: studio_lessons::Tier,
    },

    #[error("Lesson not found: {0}")]
    LessonNotFound(String),

    #[error("Insufficient data for promotion: {0}")]
    InsufficientData(String),
}

pub type Result<T> = std::result::Result<T, PromotionError>;
