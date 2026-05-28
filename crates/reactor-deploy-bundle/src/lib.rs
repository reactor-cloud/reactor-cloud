//! Reactor Deploy Bundle
//!
//! This crate provides types and utilities for creating and unpacking
//! deploy bundles used by `reactor-server` and `reactor-cli`.
//!
//! A deploy bundle is a tar.zst archive containing:
//! - `manifest.json` — describes the bundle contents and per-capability entries
//! - `migrations/` — data migrations (SQL files)
//! - `functions/` — function deployment bundles
//! - `jobs/` — job manifest entries
//! - `sites/` — site deployment bundles
//!
//! The bundle format is designed to be:
//! - Self-describing via the manifest
//! - Content-addressed (SHA-256 hashes for verification)
//! - Portable between CLI (producer) and server (consumer)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod manifest;
mod pack;
mod unpack;
mod validate;

pub use error::BundleError;
pub use manifest::{
    BundleManifest, CapabilitiesManifest, DataManifest, FunctionEntry, JobEntry, MigrationEntry,
    SiteEntry, StoragePolicyEntry,
};
pub use pack::pack;
pub use unpack::{unpack, Bundle};
pub use validate::validate;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
