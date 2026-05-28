//! Postmortem agent for Reactor Studio Foundry
//!
//! Extracts lessons from failure traces and classifies their scope
//! (project vs global).

mod error;
mod postmortem;
mod scope_classifier;

pub use error::PostmortemError;
pub use postmortem::{Postmortem, LessonCandidate};
pub use scope_classifier::ScopeClassifier;
