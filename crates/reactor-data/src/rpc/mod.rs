//! RPC function support for reactor-data.
//!
//! Allows users to define SQL functions that can be invoked via POST /data/v1/rpc/{name}.
//! Functions are defined using the Reactor SQL dialect and registered during migration apply.

mod execute;
mod store;

pub use execute::execute_rpc;
pub use store::{RpcFunction, RpcParam, RpcStore, SecurityMode};
