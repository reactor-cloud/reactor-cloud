//! HTTP route handlers.

mod admin;
mod deployments;
mod domains;
pub mod health;
mod logs;
mod metrics;
mod policies;
mod revalidate;
mod serve;

pub use admin::{create_site, delete_site, get_site, list_sites};
pub use deployments::{
    create_deployment, get_deployment, list_deployments, promote_deployment, rollback_deployment,
};
pub use domains::{create_domain, delete_domain, list_domains, verify_domain};
pub use health::{health, HealthResponse};
pub use logs::stream_logs;
pub use metrics::{metrics_handler, SiteMetrics};
pub use policies::{create_policy, delete_policy, list_policies};
pub use revalidate::revalidate;
pub use serve::serve_handler;
