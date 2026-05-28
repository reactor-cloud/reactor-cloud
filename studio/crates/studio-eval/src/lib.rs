//! Eval framework for Reactor Studio Foundry
//!
//! Provides the test runner, scorers, worktree isolation, and replay mode
//! for evaluating and improving the agent harness.

mod error;
mod loader;
mod runner;
mod scorer;
mod test;
mod worktree;
mod replay;
pub mod loop_driver;

pub use error::EvalError;
pub use loader::TestLoader;
pub use runner::{Runner, RunConfig, RunOutcome, SuiteResult};
pub use scorer::{Scorer, ScorerKind, ScoreResult};
pub use test::{Test, TestId, TestLevel, Budget, ModelPin, Fixture};
pub use worktree::WorktreeManager;
pub use replay::{ReplayMode, Cassette, CassetteEntry};
pub use loop_driver::{AutoIterationLoop, IterationReport, LoopConfig, LoopEvent, LoopState};
