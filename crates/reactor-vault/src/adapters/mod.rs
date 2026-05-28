//! Vault backend adapters.
//!
//! Each adapter implements the `Vault` trait from `reactor-core`.

#[cfg(feature = "embedded")]
pub mod embedded;

#[cfg(feature = "openbao")]
pub mod openbao;

pub mod mock;
