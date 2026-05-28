//! Core types, traits, and contracts for Reactor.cloud
//!
//! This crate defines the stable contract surface that all Reactor capabilities depend on.
//! It contains no implementations — only types, traits, and error definitions.
//!
//! # Modules
//!
//! - [`id`] — UUIDv7-based identifiers (`ReactorId`, `UserId`, `OrgId`, etc.)
//! - [`auth`] — Authentication types, traits, and errors
//! - [`error`] — Base error types and response envelope
//! - [`project`] — Project identification types (`ProjectId`, `ProjectRef`)
//! - [`tenant`] — Tenant context for multi-tenancy (`TenantCtx`, `TenantEnv`)
//! - [`primitives`] — Infrastructure primitive traits (`PubSub`, `LeaderElect`, `Vault`)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod auth;
pub mod error;
pub mod id;
pub mod primitives;
pub mod project;
pub mod tenant;

// Re-export commonly used types at the crate root
pub use id::{InvitationId, OrgId, ParseIdError, ReactorId, RoleId, SessionId, UserId};

// Re-export project and tenant types
pub use project::{ProjectId, ProjectRef, ProjectRefError, ProjectSlug};
pub use tenant::{TenantCtx, TenantEnv, TenantEnvError};

// Re-export primitive traits
pub use primitives::{
    AlwaysLeader, Ciphertext, InProcessPubSub, LeaderElect, LeaderGuard, PubSub, SecretMetadata,
    SecretValue, Subscription, Vault, VaultError,
};
