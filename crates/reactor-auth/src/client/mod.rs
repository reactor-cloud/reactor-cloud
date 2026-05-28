//! AuthClient implementations.
//!
//! Provides two implementations of `reactor_core::auth::AuthClient`:
//! - `InProcessAuthClient`: For embedded use, calls the service directly
//! - `RemoteAuthClient`: For distributed deployments, makes HTTP calls

mod in_process;
mod remote;

pub use in_process::InProcessAuthClient;
pub use remote::RemoteAuthClient;
