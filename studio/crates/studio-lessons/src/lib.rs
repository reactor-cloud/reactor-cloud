//! Lesson system for Reactor Studio Foundry
//!
//! Manages lessons - tiered, scoped behavioral artifacts that modify agent behavior
//! without changing the harness binary. Includes the lesson model, ledger, retrieval,
//! and on-disk storage.

mod error;
mod lesson;
mod ledger;
mod retriever;
mod store;

pub use error::LessonError;
pub use lesson::{Lesson, LessonId, LessonKind, Origin, Scope, Tier, Trigger, TriggerKind, PromptTarget, Constraints};
pub use ledger::{LedgerEntry, LedgerWriter, LedgerOutcome, LessonStats};
pub use retriever::{RetrievalQuery, RetrievalResult, Retriever};
pub use store::LessonStore;
