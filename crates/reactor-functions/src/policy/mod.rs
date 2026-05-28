//! Invoke-time policy evaluation.
//!
//! Integrates with `reactor-policy` to evaluate function invocation policies.

use crate::error::FunctionsError;
use crate::state::FunctionCtx;
use crate::store::Policy;
use std::collections::HashMap;

/// Request facts available to policy expressions.
#[derive(Debug, Clone)]
pub struct RequestFacts {
    /// HTTP method.
    pub method: String,
    /// Request path (sub-path after function name).
    pub path: String,
    /// Request headers.
    pub headers: HashMap<String, String>,
    /// Source IP address.
    pub source_ip: Option<String>,
}

/// Function facts available to policy expressions.
#[derive(Debug, Clone)]
pub struct FunctionFacts {
    /// Function name.
    pub name: String,
    /// Function runtime.
    pub runtime: String,
}

/// Deployment facts available to policy expressions.
#[derive(Debug, Clone)]
pub struct DeploymentFacts {
    /// Deployment version.
    pub version: i64,
    /// Deployment status.
    pub status: String,
}

/// Combined facts for policy evaluation.
pub struct InvokeFacts {
    /// Request information.
    pub request: RequestFacts,
    /// Function information.
    pub function: FunctionFacts,
    /// Deployment information.
    pub deployment: DeploymentFacts,
    /// Auth context.
    pub auth: AuthFacts,
}

/// Auth facts from FunctionCtx.
#[derive(Debug, Clone)]
pub struct AuthFacts {
    /// User ID (if present).
    pub user_id: Option<String>,
    /// Organization ID.
    pub org_id: String,
    /// Permissions.
    pub permissions: Vec<String>,
}

impl InvokeFacts {
    /// Create invoke facts from context and request data.
    pub fn new(
        ctx: &FunctionCtx,
        function_name: &str,
        function_runtime: &str,
        deployment_version: i64,
        deployment_status: &str,
        method: &str,
        path: &str,
        headers: HashMap<String, String>,
        source_ip: Option<String>,
    ) -> Self {
        Self {
            request: RequestFacts {
                method: method.to_string(),
                path: path.to_string(),
                headers,
                source_ip,
            },
            function: FunctionFacts {
                name: function_name.to_string(),
                runtime: function_runtime.to_string(),
            },
            deployment: DeploymentFacts {
                version: deployment_version,
                status: deployment_status.to_string(),
            },
            auth: AuthFacts {
                user_id: ctx.user_id().map(|id| id.to_string()),
                org_id: ctx.active_org().to_string(),
                permissions: ctx.auth.permissions.clone(),
            },
        }
    }
}

/// Policy evaluation result.
#[derive(Debug)]
pub enum PolicyDecision {
    /// Request is allowed.
    Allow,
    /// Request is denied with a reason.
    Deny {
        /// The reason for denial.
        reason: String,
    },
    /// Policy was bypassed (e.g., * permission).
    Bypass,
}

/// Evaluate policies for a function invocation.
///
/// Returns `Allow`, `Deny`, or `Bypass` based on policy evaluation.
pub fn evaluate_invoke_policies(
    policies: &[Policy],
    facts: &InvokeFacts,
    has_wildcard: bool,
) -> PolicyDecision {
    // If user has wildcard permission, bypass policy evaluation
    if has_wildcard {
        return PolicyDecision::Bypass;
    }

    // If no policies, allow by default
    if policies.is_empty() {
        return PolicyDecision::Allow;
    }

    // Build policy context
    let context = build_policy_context(facts);

    // Evaluate each policy
    for policy in policies {
        match evaluate_single_policy(policy, &context) {
            SinglePolicyResult::Deny(reason) => {
                return PolicyDecision::Deny {
                    reason: format!("policy '{}' denied: {}", policy.name, reason),
                };
            }
            SinglePolicyResult::Allow => {
                // Continue to next policy
            }
            SinglePolicyResult::Error(err) => {
                tracing::warn!(
                    policy = %policy.name,
                    error = %err,
                    "policy evaluation error, treating as deny"
                );
                return PolicyDecision::Deny {
                    reason: format!("policy '{}' evaluation error: {}", policy.name, err),
                };
            }
        }
    }

    PolicyDecision::Allow
}

enum SinglePolicyResult {
    Allow,
    Deny(String),
    Error(String),
}

fn evaluate_single_policy(policy: &Policy, _context: &serde_json::Value) -> SinglePolicyResult {
    // Parse the policy expression from JSON
    let _expr = match &policy.using_expr_json {
        Some(expr) => expr,
        None => {
            // No expression means always allow
            return SinglePolicyResult::Allow;
        }
    };

    // TODO: Actually evaluate the policy expression using reactor-policy
    // For now, just check for simple patterns in the raw text
    let raw = &policy.raw_text;
    if raw.contains("DENY") || raw.contains("deny") {
        SinglePolicyResult::Deny("policy contains explicit DENY".to_string())
    } else {
        SinglePolicyResult::Allow
    }
}

fn build_policy_context(facts: &InvokeFacts) -> serde_json::Value {
    serde_json::json!({
        "request": {
            "method": facts.request.method,
            "path": facts.request.path,
            "headers": facts.request.headers,
            "source_ip": facts.request.source_ip,
        },
        "function": {
            "name": facts.function.name,
            "runtime": facts.function.runtime,
        },
        "deployment": {
            "version": facts.deployment.version,
            "status": facts.deployment.status,
        },
        "auth": {
            "user_id": facts.auth.user_id,
            "org_id": facts.auth.org_id,
            "permissions": facts.auth.permissions,
        }
    })
}

/// Policy routes for managing function policies.
pub mod routes {
    use super::*;
    use axum::{
        extract::{Extension, Path, State},
        http::StatusCode,
        response::IntoResponse,
        Json,
    };
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};

    use crate::state::FunctionsState;
    use crate::store::{FunctionsStore, PgFunctionsStore};

    /// Request to create a policy.
    #[derive(Debug, Deserialize)]
    pub struct CreatePolicyRequest {
        /// Policy name.
        pub name: String,
        /// Policy expression text.
        pub expression: String,
    }

    /// Response for a policy.
    #[derive(Debug, Serialize)]
    pub struct PolicyResponse {
        /// Policy ID.
        pub id: String,
        /// Policy name.
        pub name: String,
        /// Raw policy text.
        pub expression: String,
        /// SHA256 hash of the policy.
        pub sha256: String,
        /// Created timestamp.
        pub created_at: String,
    }

    /// Response for listing policies.
    #[derive(Debug, Serialize)]
    pub struct ListPoliciesResponse {
        /// List of policies.
        pub policies: Vec<PolicyResponse>,
    }

    /// Path parameters for policy routes.
    #[derive(Debug, Deserialize)]
    pub struct PolicyPathParams {
        /// Function name.
        pub name: String,
    }

    /// Path parameters for single policy routes.
    #[derive(Debug, Deserialize)]
    pub struct PolicyNamePathParams {
        /// Function name.
        pub name: String,
        /// Policy name.
        pub policy_name: String,
    }

    /// POST /fn/v1/_admin/functions/{name}/policies
    ///
    /// Create a new policy for a function.
    pub async fn create_policy(
        State(state): State<FunctionsState>,
        Extension(ctx): Extension<FunctionCtx>,
        Path(params): Path<PolicyPathParams>,
        Json(body): Json<CreatePolicyRequest>,
    ) -> Result<impl IntoResponse, FunctionsError> {
        // Check permission
        let permission = format!("functions:{}:admin", params.name);
        if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
            return Err(FunctionsError::PermissionDenied(permission));
        }

        // Get the function
        let store = PgFunctionsStore::new(state.pool.clone());
        let function = store
            .get_function_by_name(ctx.active_org(), &params.name)
            .await?
            .ok_or_else(|| FunctionsError::FunctionNotFound(params.name.clone()))?;

        // TODO: Parse and validate the policy expression using reactor-policy
        // For now, just store the raw text
        let using_expr_json = None;

        // Compute SHA256 of the policy text
        let mut hasher = Sha256::new();
        hasher.update(body.expression.as_bytes());
        let sha256: [u8; 32] = hasher.finalize().into();

        // Create the policy
        let policy = store
            .create_policy(
                function.id,
                &body.name,
                using_expr_json,
                &body.expression,
                sha256.to_vec(),
            )
            .await?;

        // TODO: PR 14 - Record audit event

        let response = PolicyResponse {
            id: policy.id.to_string(),
            name: policy.name,
            expression: policy.raw_text,
            sha256: hex::encode(&policy.sha256),
            created_at: policy.created_at.to_rfc3339(),
        };

        Ok((StatusCode::CREATED, Json(response)))
    }

    /// GET /fn/v1/_admin/functions/{name}/policies
    ///
    /// List all policies for a function.
    pub async fn list_policies(
        State(state): State<FunctionsState>,
        Extension(ctx): Extension<FunctionCtx>,
        Path(params): Path<PolicyPathParams>,
    ) -> Result<impl IntoResponse, FunctionsError> {
        // Check permission
        let permission = format!("functions:{}:admin", params.name);
        if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
            return Err(FunctionsError::PermissionDenied(permission));
        }

        // Get the function
        let store = PgFunctionsStore::new(state.pool.clone());
        let function = store
            .get_function_by_name(ctx.active_org(), &params.name)
            .await?
            .ok_or_else(|| FunctionsError::FunctionNotFound(params.name.clone()))?;

        // Get policies
        let policies = store.get_policies(function.id).await?;

        let response = ListPoliciesResponse {
            policies: policies
                .into_iter()
                .map(|p| PolicyResponse {
                    id: p.id.to_string(),
                    name: p.name,
                    expression: p.raw_text,
                    sha256: hex::encode(&p.sha256),
                    created_at: p.created_at.to_rfc3339(),
                })
                .collect(),
        };

        Ok(Json(response))
    }

    /// DELETE /fn/v1/_admin/functions/{name}/policies/{policy_name}
    ///
    /// Delete a policy.
    pub async fn delete_policy(
        State(state): State<FunctionsState>,
        Extension(ctx): Extension<FunctionCtx>,
        Path(params): Path<PolicyNamePathParams>,
    ) -> Result<impl IntoResponse, FunctionsError> {
        // Check permission
        let permission = format!("functions:{}:admin", params.name);
        if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
            return Err(FunctionsError::PermissionDenied(permission));
        }

        // Get the function
        let store = PgFunctionsStore::new(state.pool.clone());
        let function = store
            .get_function_by_name(ctx.active_org(), &params.name)
            .await?
            .ok_or_else(|| FunctionsError::FunctionNotFound(params.name.clone()))?;

        // Delete the policy
        let deleted = store.delete_policy(function.id, &params.policy_name).await?;
        if !deleted {
            return Err(FunctionsError::InvalidRequest(format!(
                "policy '{}' not found",
                params.policy_name
            )));
        }

        // TODO: PR 14 - Record audit event

        Ok(StatusCode::NO_CONTENT)
    }
}
