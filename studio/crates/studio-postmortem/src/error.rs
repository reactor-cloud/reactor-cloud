use thiserror::Error;

#[derive(Error, Debug)]
pub enum PostmortemError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Lesson error: {0}")]
    Lesson(#[from] studio_lessons::LessonError),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Classification failed: {0}")]
    ClassificationFailed(String),

    #[error("No lessons extracted from trace")]
    NoLessonsExtracted,
}

pub type Result<T> = std::result::Result<T, PostmortemError>;
