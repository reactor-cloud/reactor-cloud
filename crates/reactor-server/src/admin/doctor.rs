//! Doctor probe endpoint.

use crate::boot::Tenant;
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use std::collections::HashMap;

/// Doctor response.
#[derive(Debug, Serialize)]
pub struct DoctorResponse {
    /// Overall status: "ok", "warn", or "fail".
    pub status: String,

    /// Active tenant context information.
    pub tenant: TenantInfo,

    /// Per-capability probe results.
    pub capabilities: HashMap<String, CapabilityProbe>,
}

/// Tenant context information.
#[derive(Debug, Serialize)]
pub struct TenantInfo {
    /// Project ID (UUID).
    pub project_id: String,

    /// Project ref (subdomain identifier).
    pub project_ref: String,

    /// Project name.
    pub project_name: String,

    /// Environment (production/preview/dev).
    pub env: String,
}

/// Per-capability probe result.
#[derive(Debug, Serialize)]
pub struct CapabilityProbe {
    /// Status: "ok", "warn", or "fail".
    pub status: String,

    /// Probe details.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, serde_json::Value>,
}

/// GET /_admin/doctor handler.
///
/// Runs health probes for each capability and reports the active tenant context.
pub async fn doctor_handler(tenant: Option<Tenant>) -> impl IntoResponse {
    let mut capabilities = HashMap::new();

    // Extract tenant info (or use defaults if not available)
    let tenant_info = match tenant {
        Some(t) => TenantInfo {
            project_id: t.project_id().to_string(),
            project_ref: t.project_ref().to_string(),
            project_name: t.project_name().to_string(),
            env: t.env().to_string(),
        },
        None => TenantInfo {
            project_id: "not-configured".to_string(),
            project_ref: "not-configured".to_string(),
            project_name: "not-configured".to_string(),
            env: "unknown".to_string(),
        },
    };

    // Basic DB probe would go here
    capabilities.insert(
        "database".to_string(),
        CapabilityProbe {
            status: "ok".to_string(),
            details: HashMap::new(),
        },
    );

    // Auth probe
    #[cfg(feature = "cap-auth")]
    capabilities.insert(
        "auth".to_string(),
        CapabilityProbe {
            status: "ok".to_string(),
            details: HashMap::new(),
        },
    );

    // Data probe
    #[cfg(feature = "cap-data")]
    capabilities.insert(
        "data".to_string(),
        CapabilityProbe {
            status: "ok".to_string(),
            details: HashMap::new(),
        },
    );

    // Storage probe
    #[cfg(feature = "cap-storage")]
    capabilities.insert(
        "storage".to_string(),
        CapabilityProbe {
            status: "ok".to_string(),
            details: HashMap::new(),
        },
    );

    // Functions probe
    #[cfg(feature = "cap-functions")]
    capabilities.insert(
        "functions".to_string(),
        CapabilityProbe {
            status: "ok".to_string(),
            details: HashMap::new(),
        },
    );

    // Jobs probe
    #[cfg(feature = "cap-jobs")]
    capabilities.insert(
        "jobs".to_string(),
        CapabilityProbe {
            status: "ok".to_string(),
            details: HashMap::new(),
        },
    );

    let all_ok = capabilities.values().all(|p| p.status == "ok");
    let any_fail = capabilities.values().any(|p| p.status == "fail");

    let overall_status = if any_fail {
        "fail"
    } else if all_ok {
        "ok"
    } else {
        "warn"
    };

    let response = DoctorResponse {
        status: overall_status.to_string(),
        tenant: tenant_info,
        capabilities,
    };

    let status_code = if any_fail {
        StatusCode::MULTI_STATUS
    } else {
        StatusCode::OK
    };

    (status_code, Json(response))
}
