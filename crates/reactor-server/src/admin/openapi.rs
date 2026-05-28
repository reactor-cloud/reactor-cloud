//! Merged OpenAPI specification handler.
//!
//! Combines OpenAPI specs from all enabled capability crates into a single
//! unified specification served at `/_api/openapi.json`.

use axum::{http::StatusCode, response::IntoResponse, Json};
use utoipa::openapi::{
    info::{ContactBuilder, InfoBuilder, LicenseBuilder},
    OpenApi,
};

/// Handler for the merged OpenAPI specification endpoint.
///
/// Returns a unified OpenAPI spec combining all enabled capabilities.
pub async fn openapi_handler() -> impl IntoResponse {
    let spec = build_merged_spec();
    (StatusCode::OK, Json(spec))
}

/// Build the merged OpenAPI specification from all enabled capability crates.
fn build_merged_spec() -> OpenApi {
    let info = InfoBuilder::new()
        .title("Reactor API")
        .version(crate::VERSION)
        .description(Some(
            "Unified Reactor backend API combining auth, data, storage, functions, jobs, and sites capabilities.",
        ))
        .contact(Some(
            ContactBuilder::new()
                .name(Some("Reactor"))
                .url(Some("https://reactor.cloud"))
                .build(),
        ))
        .license(Some(
            LicenseBuilder::new()
                .name("MIT")
                .url(Some("https://opensource.org/licenses/MIT"))
                .build(),
        ))
        .build();

    let mut merged = OpenApi::new(info, utoipa::openapi::Paths::default());

    // Merge auth capability spec
    #[cfg(feature = "cap-auth")]
    {
        let auth_spec = reactor_auth::openapi();
        merge_spec(&mut merged, auth_spec);
    }

    // Merge data capability spec
    #[cfg(feature = "cap-data")]
    {
        let data_spec = reactor_data::openapi();
        merge_spec(&mut merged, data_spec);
    }

    // Merge storage capability spec
    #[cfg(feature = "cap-storage")]
    {
        let storage_spec = reactor_storage::openapi();
        merge_spec(&mut merged, storage_spec);
    }

    // Merge functions capability spec
    #[cfg(feature = "cap-functions")]
    {
        let functions_spec = reactor_functions::openapi();
        merge_spec(&mut merged, functions_spec);
    }

    // Merge jobs capability spec
    #[cfg(feature = "cap-jobs")]
    {
        let jobs_spec = reactor_jobs::openapi();
        merge_spec(&mut merged, jobs_spec);
    }

    // Merge sites capability spec
    #[cfg(feature = "cap-sites")]
    {
        let sites_spec = reactor_sites::openapi();
        merge_spec(&mut merged, sites_spec);
    }

    merged
}

/// Merge a capability spec into the main spec.
fn merge_spec(target: &mut OpenApi, source: OpenApi) {
    // Merge paths
    for (path, item) in source.paths.paths {
        target.paths.paths.insert(path, item);
    }

    // Merge components (schemas, security schemes, etc.)
    if let Some(source_components) = source.components {
        if let Some(ref mut target_components) = target.components {
            // Merge schemas (BTreeMap in utoipa 5)
            for (name, schema) in source_components.schemas {
                target_components.schemas.insert(name, schema);
            }

            // Merge security schemes (BTreeMap in utoipa 5)
            for (name, scheme) in source_components.security_schemes {
                target_components.security_schemes.insert(name, scheme);
            }
        } else {
            target.components = Some(source_components);
        }
    }

    // Merge tags
    if let Some(source_tags) = source.tags {
        if let Some(ref mut target_tags) = target.tags {
            for tag in source_tags {
                if !target_tags.iter().any(|t| t.name == tag.name) {
                    target_tags.push(tag);
                }
            }
        } else {
            target.tags = Some(source_tags);
        }
    }
}
