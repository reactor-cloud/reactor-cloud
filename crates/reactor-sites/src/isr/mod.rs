//! ISR (Incremental Static Regeneration) support.

pub mod cache;
pub mod revalidate;

pub use cache::IsrCache;
pub use revalidate::RevalidationManager;
