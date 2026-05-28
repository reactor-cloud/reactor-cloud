//! Reactor Vault — Secret management with Embedded and OpenBao backends.
//!
//! This crate implements the [`Vault`] trait from `reactor-core` with two backends:
//!
//! - **Embedded** — File-based AES-GCM encryption for single-node deployments (G1/G2)
//! - **OpenBao** — HashiCorp Vault-compatible backend for production (G3)
//!
//! # Features
//!
//! - `embedded` (default) — File-based vault for development and single-node
//! - `openbao` — OpenBao/Vault client for production deployments
//!
//! # Usage
//!
//! ```ignore
//! use reactor_vault::{EmbeddedVault, VaultConfig};
//!
//! // For development/single-node
//! let vault = EmbeddedVault::new("/data/vault", "master_key_from_env").await?;
//!
//! // For production with OpenBao
//! let vault = OpenBaoVault::new(config).await?;
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod adapters;
mod config;
mod error;

pub use config::{EmbeddedConfig, OpenBaoAuth, OpenBaoConfig, VaultConfig};
pub use error::VaultError;

// Re-export the Vault trait and types from reactor-core
pub use reactor_core::primitives::vault::{Ciphertext, SecretMetadata, SecretValue, Vault};

#[cfg(feature = "embedded")]
pub use adapters::embedded::EmbeddedVault;

#[cfg(feature = "openbao")]
pub use adapters::openbao::OpenBaoVault;

pub use adapters::mock::MockVault;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
