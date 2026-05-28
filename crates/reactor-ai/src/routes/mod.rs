//! HTTP route handlers for the AI capability.

pub mod chat;
pub mod embeddings;
pub mod health;
pub mod models;

pub use health::health;
