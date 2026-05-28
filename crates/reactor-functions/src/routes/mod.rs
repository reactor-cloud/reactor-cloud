//! HTTP route handlers.

mod admin;
mod deployments;
mod env;
pub mod health;
mod invoke;
mod logs;
mod metrics;

pub use admin::{
    create_function, delete_function, get_function, list_functions, CreateFunctionRequest,
    FunctionResponse, ListFunctionsResponse,
};
pub use deployments::{
    create_deployment, get_deployment, list_deployments, promote_deployment, rollback_deployment,
    DeploymentResponse, ListDeploymentsResponse, PromoteResponse,
};
pub use env::{delete_env, get_env, list_env, set_env, decrypt_value, load_env_for_invoke};
pub use health::{health, HealthResponse};
pub use invoke::{invoke_handler, InvokeParams};
pub use logs::{stream_logs, LogEvent};
pub use metrics::{metrics_handler, FunctionMetrics};
