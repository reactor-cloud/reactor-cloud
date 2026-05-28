//! Operations control surface for Reactor platform operators.
//!
//! This crate provides the `/_ops/v1/*` HTTP surface for secure,
//! authenticated, and audited control plane operations.
//!
//! ## Key Features
//!
//! - **Network layer**: Trusted networks allowlist (loopback + Fly 6PN by default)
//! - **Identity layer**: JWT-based authentication via reactor-auth
//! - **Scope layer**: Fine-grained permission scopes (ops:deploy, ops:cluster_admin, etc.)
//! - **Step-up layer**: MFA/WebAuthn step-up for high-risk operations
//! - **Audit layer**: Full audit trail with real actor attribution
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐
//! │  CLI/Client │
//! └─────┬───────┘
//!       │ Authorization: Bearer <JWT>
//!       ▼
//! ┌─────────────┐
//! │  Network    │  ← trusted_networks allowlist
//! └─────┬───────┘
//!       ▼
//! ┌─────────────┐
//! │  Identity   │  ← JWT validation via AuthClient
//! └─────┬───────┘
//!       ▼
//! ┌─────────────┐
//! │  Scope      │  ← route metadata declares required scope
//! └─────┬───────┘
//!       ▼
//! ┌─────────────┐
//! │  Step-up    │  ← check mfa_at for flagged scopes
//! └─────┬───────┘
//!       ▼
//! ┌─────────────┐
//! │  Handler    │  ← actual operation (deploy, project create, etc.)
//! └─────┬───────┘
//!       ▼
//! ┌─────────────┐
//! │  Audit      │  ← write ops_audit_log with actor, action, result
//! └─────────────┘
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod config;
pub mod error;
pub mod middleware;
pub mod router;
pub mod routes;
pub mod state;

pub use config::OpsConfig;
pub use error::OpsError;
pub use router::router;
pub use state::OpsState;
