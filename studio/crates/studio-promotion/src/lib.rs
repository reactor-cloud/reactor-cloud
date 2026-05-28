//! Promotion system for Reactor Studio Foundry
//!
//! Handles tier transitions (T0->T1->T2), demotion rules,
//! and rollback gate when iterations regress pass rate.

mod error;
mod promoter;
mod rollback;

pub use error::PromotionError;
pub use promoter::{Promoter, TierTransition};
pub use rollback::RollbackGate;
