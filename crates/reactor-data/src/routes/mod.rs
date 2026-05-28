//! HTTP route handlers for reactor-data.

pub mod admin;
pub mod crud;
pub mod health;
pub mod metrics;
pub mod rpc;

pub use admin::generate_typescript;
pub use crud::{delete_table, get_table, patch_table, post_table};
pub use health::health;
pub use metrics::metrics;
pub use rpc::post_rpc;
