// Task management for Reactor Studio
// Implements the phased task workflow: Alignment → Planning → Development → Testing → UAT → Deployment

mod error;
mod store;
mod types;

pub use error::TaskError;
pub use store::TaskStore;
pub use types::{Phase, PhaseStatus, Task, TaskId, TaskSummary, TaskState};
