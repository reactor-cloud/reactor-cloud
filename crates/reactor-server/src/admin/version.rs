//! Version endpoint.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use std::collections::HashMap;

/// Version response.
#[derive(Debug, Serialize)]
pub struct VersionResponse {
    /// Server version.
    pub reactor_server: String,

    /// Per-capability versions.
    pub capabilities: HashMap<String, String>,
}

/// Version handler.
///
/// Returns version information for the server and all capabilities.
pub async fn version_handler() -> impl IntoResponse {
    let mut capabilities = HashMap::new();

    #[cfg(feature = "cap-auth")]
    capabilities.insert("auth".to_string(), reactor_auth::VERSION.to_string());

    #[cfg(feature = "cap-data")]
    capabilities.insert("data".to_string(), reactor_data::VERSION.to_string());

    #[cfg(feature = "cap-storage")]
    capabilities.insert("storage".to_string(), reactor_storage::VERSION.to_string());

    #[cfg(feature = "cap-functions")]
    capabilities.insert(
        "functions".to_string(),
        reactor_functions::VERSION.to_string(),
    );

    #[cfg(feature = "cap-jobs")]
    capabilities.insert("jobs".to_string(), reactor_jobs::VERSION.to_string());

    #[cfg(feature = "cap-sites")]
    capabilities.insert("sites".to_string(), reactor_sites::VERSION.to_string());

    let response = VersionResponse {
        reactor_server: crate::VERSION.to_string(),
        capabilities,
    };

    (StatusCode::OK, Json(response))
}
