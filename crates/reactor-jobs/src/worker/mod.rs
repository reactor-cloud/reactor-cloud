//! Worker pool for job execution.

pub mod checkpoint;
pub mod executor;
pub mod pool;

pub use pool::WorkerPool;
