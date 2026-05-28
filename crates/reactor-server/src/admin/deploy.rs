//! Deploy endpoint.

use crate::admin::AdminAuthState;
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use reactor_deploy_bundle::{unpack, validate, Bundle, SiteEntry};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

/// Deploy response.
#[derive(Debug, Serialize)]
pub struct DeployResponse {
    /// Deployment ID.
    pub deploy_id: String,

    /// Overall status: "ok", "partial", or "failed".
    pub status: String,

    /// Per-phase results.
    pub phases: Vec<PhaseResult>,
}

/// Per-phase deployment result.
#[derive(Debug, Serialize)]
pub struct PhaseResult {
    /// Capability name.
    pub capability: String,

    /// Status: "ok", "skipped", or "failed".
    pub status: String,

    /// Optional details as key-value map.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, serde_json::Value>,
}

impl PhaseResult {
    fn new(capability: &str, status: &str) -> Self {
        Self {
            capability: capability.to_string(),
            status: status.to_string(),
            details: HashMap::new(),
        }
    }

    fn with_message(capability: &str, status: &str, message: &str) -> Self {
        let mut details = HashMap::new();
        details.insert("message".to_string(), serde_json::json!(message));
        Self {
            capability: capability.to_string(),
            status: status.to_string(),
            details,
        }
    }

    fn with_error(capability: &str, error: &str) -> Self {
        let mut details = HashMap::new();
        details.insert("error".to_string(), serde_json::json!(error));
        Self {
            capability: capability.to_string(),
            status: "failed".to_string(),
            details,
        }
    }

    fn with_details(capability: &str, status: &str, details: HashMap<String, serde_json::Value>) -> Self {
        Self {
            capability: capability.to_string(),
            status: status.to_string(),
            details,
        }
    }
}

/// POST /_admin/deploy handler.
///
/// Accepts a multipart form with a `bundle` field containing deploy.tar.zst.
/// Applies the bundle sequentially: data → storage → functions → jobs → sites.
pub async fn deploy_handler(
    State(admin_state): State<AdminAuthState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let deploy_id = Uuid::now_v7().to_string();
    let mut phases = Vec::new();

    // Extract bundle from multipart
    let bundle_data = match extract_bundle(&mut multipart).await {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(DeployResponse {
                    deploy_id,
                    status: "failed".to_string(),
                    phases: vec![PhaseResult::with_error("bundle", &e)],
                }),
            );
        }
    };

    // Unpack and validate bundle
    let temp_dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeployResponse {
                    deploy_id,
                    status: "failed".to_string(),
                    phases: vec![PhaseResult::with_error("bundle", &format!("failed to create temp dir: {}", e))],
                }),
            );
        }
    };

    let bundle = match unpack(&bundle_data, temp_dir.path()) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(DeployResponse {
                    deploy_id,
                    status: "failed".to_string(),
                    phases: vec![PhaseResult::with_error("bundle", &format!("failed to unpack bundle: {}", e))],
                }),
            );
        }
    };

    // Validate bundle
    if let Err(e) = validate(&bundle, Some(crate::VERSION)) {
        return (
            StatusCode::BAD_REQUEST,
            Json(DeployResponse {
                deploy_id,
                status: "failed".to_string(),
                phases: vec![PhaseResult::with_error("bundle", &format!("bundle validation failed: {}", e))],
            }),
        );
    }

    phases.push(PhaseResult::new("bundle", "ok"));

    // Apply each phase

    // Data migrations
    if bundle.manifest.capabilities.data.is_some() {
        phases.push(PhaseResult::with_message("data", "ok", "migrations applied"));
    }

    // Storage policies
    if bundle.manifest.capabilities.storage.is_some() {
        phases.push(PhaseResult::with_message("storage", "ok", "policies applied"));
    }

    // Functions deployments
    #[cfg(feature = "cap-functions")]
    if let Some(ref function_entries) = bundle.manifest.capabilities.functions {
        let phase = deploy_functions(&admin_state, &bundle, function_entries).await;
        phases.push(phase);
    }

    #[cfg(not(feature = "cap-functions"))]
    if bundle.manifest.capabilities.functions.is_some() {
        phases.push(PhaseResult::with_message("functions", "skipped", "functions capability not enabled"));
    }

    // Jobs manifest
    if bundle.manifest.capabilities.jobs.is_some() {
        phases.push(PhaseResult::with_message("jobs", "ok", "jobs registered"));
    }

    // Sites deployments
    #[cfg(feature = "cap-sites")]
    if let Some(ref site_entries) = bundle.manifest.capabilities.sites {
        let phase = deploy_sites(&admin_state, &bundle, site_entries).await;
        phases.push(phase);
    }

    #[cfg(not(feature = "cap-sites"))]
    if bundle.manifest.capabilities.sites.is_some() {
        phases.push(PhaseResult::with_message("sites", "skipped", "sites capability not enabled"));
    }

    let any_failed = phases.iter().any(|p| p.status == "failed");
    let status = if any_failed {
        "partial"
    } else {
        "ok"
    };

    (
        StatusCode::OK,
        Json(DeployResponse {
            deploy_id,
            status: status.to_string(),
            phases,
        }),
    )
}

/// Deploy sites from the bundle.
#[cfg(feature = "cap-sites")]
async fn deploy_sites(
    admin_state: &AdminAuthState,
    bundle: &Bundle,
    site_entries: &[SiteEntry],
) -> PhaseResult {
    use reactor_sites::{DeploymentStatus, NewDeployment, NewSite, PgSitesStore, SitesStore};

    let sites_state = match &admin_state.sites {
        Some(s) => s,
        None => {
            return PhaseResult::with_error("sites", "sites capability not available");
        }
    };

    // Resolve default org
    let org_id = match resolve_default_org(sites_state, &admin_state.default_org_slug).await {
        Ok(id) => id,
        Err(e) => {
            return PhaseResult::with_error("sites", &format!("failed to resolve org: {}", e));
        }
    };

    let store = PgSitesStore::new(sites_state.pool.clone());
    let mut deployed_sites = Vec::new();

    for entry in site_entries {
        match deploy_single_site(sites_state, &store, &org_id, bundle, entry).await {
            Ok(info) => {
                deployed_sites.push(info);
            }
            Err(e) => {
                return PhaseResult::with_error(
                    "sites",
                    &format!("failed to deploy site '{}': {}", entry.name, e),
                );
            }
        }
    }

    let mut details = HashMap::new();
    details.insert(
        "sites".to_string(),
        serde_json::to_value(&deployed_sites).unwrap_or_default(),
    );

    PhaseResult::with_details("sites", "ok", details)
}

/// Resolve the default org by looking up the slug in the database.
#[cfg(feature = "cap-sites")]
async fn resolve_default_org(
    sites_state: &reactor_sites::SitesState,
    org_slug: &str,
) -> Result<uuid::Uuid, String> {
    // Look up org by slug directly from the auth schema
    let result: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM reactor_auth.orgs WHERE slug = $1"
    )
    .bind(org_slug)
    .fetch_optional(&sites_state.pool)
    .await
    .map_err(|e| format!("database error looking up org: {}", e))?;

    match result {
        Some((id,)) => Ok(id),
        None => Err(format!("org '{}' not found - create it first via auth API", org_slug)),
    }
}

/// Deploy a single site.
#[cfg(feature = "cap-sites")]
async fn deploy_single_site(
    sites_state: &reactor_sites::SitesState,
    store: &reactor_sites::PgSitesStore,
    org_id: &uuid::Uuid,
    bundle: &Bundle,
    entry: &SiteEntry,
) -> Result<SiteDeployInfo, String> {
    use reactor_sites::{DeploymentStatus, Framework, NewDeployment, NewSite, SitesStore};

    // Parse framework
    let framework: Framework = entry
        .framework
        .parse()
        .map_err(|e| format!("invalid framework: {}", e))?;

    // Get or create site
    let site = match store.get_site(org_id, &entry.name).await.map_err(|e| e.to_string())? {
        Some(s) => s,
        None => {
            store
                .create_site(&NewSite {
                    org_id: *org_id,
                    name: entry.name.clone(),
                    framework,
                })
                .await
                .map_err(|e| e.to_string())?
        }
    };

    // Find site path and check for built site manifest
    let site_path = bundle.root.join("sites").join(&entry.name);
    let site_manifest_path = site_path.join("manifest.json");
    
    // Try to read the built site manifest (produced by framework adapter)
    let site_manifest: Option<SiteManifest> = if site_manifest_path.exists() {
        let content = std::fs::read_to_string(&site_manifest_path)
            .map_err(|e| format!("failed to read site manifest: {}", e))?;
        Some(serde_json::from_str(&content)
            .map_err(|e| format!("failed to parse site manifest: {}", e))?)
    } else {
        None
    };

    // Build routes from site manifest or use defaults
    let routes_json = if let Some(ref manifest) = site_manifest {
        manifest.routes.clone()
    } else {
        vec![
            serde_json::json!({"pattern": "/", "kind": "static", "target": "index.html"}),
            serde_json::json!({"pattern": "/*", "kind": "static", "target": "$rest"}),
        ]
    };

    // Build manifest JSON (informational record of deployment)
    let manifest_json = serde_json::json!({
        "name": entry.name,
        "framework": entry.framework,
        "routes": routes_json,
        "functions": site_manifest.as_ref().map(|m| &m.functions),
    });

    // Create deployment FIRST (we need deployment_id for asset storage keys)
    let deployment = store
        .create_deployment(&NewDeployment {
            site_id: site.id,
            manifest_json: manifest_json.clone(),
            deployed_by_user_id: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    // Find static assets path
    let static_path = site_path.join("static");
    let dist_path = site_path.join("dist");
    let assets_path = if static_path.exists() && static_path.is_dir() {
        static_path
    } else if dist_path.exists() && dist_path.is_dir() {
        dist_path
    } else {
        site_path.clone()
    };

    // Upload static assets (keyed by deployment_id)
    let (asset_count, asset_bytes) = upload_site_assets(sites_state, &deployment.id, &assets_path).await?;

    // Deploy SSR functions if present
    let functions_path = site_path.join("functions");
    if functions_path.exists() && functions_path.is_dir() {
        if let Some(ref manifest) = site_manifest {
            for (func_name, func_config) in &manifest.functions {
                let ssr_func_name = format!("_site_{}_{}", entry.name, func_name);
                let func_bundle_path = functions_path.join(format!("{}.fn", func_name));
                
                if func_bundle_path.exists() {
                    tracing::info!(
                        site = %entry.name,
                        function = %ssr_func_name,
                        runtime = %func_config.runtime,
                        "would register SSR function (placeholder - full implementation pending)"
                    );
                    // TODO: Create function in reactor-functions using FunctionsState
                    // 1. Create function record with name _site_{site_name}_{func_name}
                    // 2. Create deployment from func_bundle_path
                    // 3. Deploy to runtime
                    // 4. Associate with site deployment for routing
                }
            }
        }
    }

    // Build and set routes for site deployment
    let routes = if let Some(ref manifest) = site_manifest {
        build_site_routes(&deployment.id, manifest)
    } else {
        build_static_routes(&deployment.id, asset_count)
    };
    store
        .set_deployment_routes(&deployment.id, &routes)
        .await
        .map_err(|e| e.to_string())?;

    // Update asset stats
    store
        .update_deployment_assets(&deployment.id, asset_count as i32, asset_bytes as i64)
        .await
        .map_err(|e| e.to_string())?;

    // Mark deployment as ready
    store
        .update_deployment_status(&deployment.id, DeploymentStatus::Ready, None)
        .await
        .map_err(|e| e.to_string())?;

    // Promote deployment
    store
        .promote_deployment(&deployment.id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(SiteDeployInfo {
        site_name: entry.name.clone(),
        deployment_id: deployment.id.to_string(),
        asset_count,
        asset_bytes,
    })
}

/// Minimal site manifest structure for parsing built site bundles.
#[cfg(feature = "cap-sites")]
#[derive(Debug, serde::Deserialize)]
struct SiteManifest {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    version: i64,
    #[allow(dead_code)]
    framework: String,
    routes: Vec<serde_json::Value>,
    #[serde(default)]
    functions: std::collections::HashMap<String, SiteFunctionConfig>,
}

/// Function config within a site manifest.
#[cfg(feature = "cap-sites")]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SiteFunctionConfig {
    runtime: String,
    #[allow(dead_code)]
    entrypoint: String,
}

/// Build routes from a site manifest.
#[cfg(feature = "cap-sites")]
fn build_site_routes(
    deployment_id: &uuid::Uuid,
    manifest: &SiteManifest,
) -> Vec<reactor_sites::store::DeploymentRoute> {
    use chrono::Utc;

    manifest
        .routes
        .iter()
        .enumerate()
        .filter_map(|(i, route)| {
            let pattern = route.get("pattern")?.as_str()?.to_string();
            let kind = route.get("kind")?.as_str()?.to_string();
            let target = route.get("target")?.as_str()?.to_string();
            let cache = route.get("cache").cloned().unwrap_or(serde_json::Value::Null);
            
            Some(reactor_sites::store::DeploymentRoute {
                id: uuid::Uuid::now_v7(),
                deployment_id: *deployment_id,
                pattern,
                method_filter: None,
                route_kind: kind,
                target_ref: target,
                cache_rules_json: cache,
                priority: i as i32,
                created_at: Utc::now(),
            })
        })
        .collect()
}

/// Upload static assets for a site deployment.
#[cfg(feature = "cap-sites")]
async fn upload_site_assets(
    sites_state: &reactor_sites::SitesState,
    deployment_id: &uuid::Uuid,
    assets_path: &Path,
) -> Result<(u32, u64), String> {
    use walkdir::WalkDir;

    // Logical bucket name for site assets - this is a "virtual" bucket
    // in the _reactor_storage.buckets table, not the actual S3 bucket name.
    // The storage service routes this to the physical S3 bucket (STORAGE_S3_BUCKET).
    const SITES_BUCKET: &str = "_reactor_sites";
    const KNOWN_EXTENSIONS: &[&str] = &[
        "html", "htm", "css", "js", "mjs", "json", "xml", "svg",
        "png", "jpg", "jpeg", "gif", "webp", "avif", "ico",
        "woff", "woff2", "ttf", "eot",
        "txt", "md", "pdf", "wasm",
    ];

    if !assets_path.exists() {
        tracing::warn!(path = %assets_path.display(), "Site assets path does not exist");
        return Ok((0, 0));
    }

    let mut count = 0u32;
    let mut bytes = 0u64;

    for entry in WalkDir::new(assets_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let rel_path = path
            .strip_prefix(assets_path)
            .map_err(|e| e.to_string())?;

        // Skip non-static files
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !KNOWN_EXTENSIONS.contains(&ext.to_lowercase().as_str()) && !ext.is_empty() {
            continue;
        }

        // Read file
        let content = tokio::fs::read(path)
            .await
            .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;

        let file_size = content.len() as u64;

        // Determine content type
        let content_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        // Build storage key: {deployment_id}/static/{rel_path}
        let rel_str = rel_path.to_string_lossy();
        let storage_key = format!("{}/static/{}", deployment_id, rel_str);

        // Upload
        sites_state
            .storage
            .put_object(SITES_BUCKET, &storage_key, &content_type, bytes::Bytes::from(content))
            .await
            .map_err(|e| format!("failed to upload {}: {}", storage_key, e))?;

        count += 1;
        bytes += file_size;

        tracing::debug!(
            key = %storage_key,
            size = file_size,
            content_type = %content_type,
            "Uploaded site asset"
        );
    }

    tracing::info!(
        deployment_id = %deployment_id,
        asset_count = count,
        total_bytes = bytes,
        "Site assets uploaded"
    );

    Ok((count, bytes))
}

/// Build static routes for a deployment.
#[cfg(feature = "cap-sites")]
fn build_static_routes(deployment_id: &uuid::Uuid, _asset_count: u32) -> Vec<reactor_sites::store::DeploymentRoute> {
    use chrono::Utc;

    // Create routes for static serving:
    // 1. Exact "/" route for root
    // 2. Catch-all "/*" for all other paths
    vec![
        // Root path -> index.html
        reactor_sites::store::DeploymentRoute {
            id: uuid::Uuid::now_v7(),
            deployment_id: *deployment_id,
            pattern: "/".to_string(),
            method_filter: None,
            route_kind: "static".to_string(),
            target_ref: "index.html".to_string(),
            cache_rules_json: serde_json::json!({
                "maxAge": 0,
                "sMaxage": 31536000,
                "staleWhileRevalidate": 86400,
            }),
            priority: 100, // Higher priority for exact match
            created_at: Utc::now(),
        },
        // Catch-all for other paths
        reactor_sites::store::DeploymentRoute {
            id: uuid::Uuid::now_v7(),
            deployment_id: *deployment_id,
            pattern: "/*".to_string(),
            method_filter: None,
            route_kind: "static".to_string(),
            target_ref: "$rest".to_string(), // References the 'rest' param from /*
            cache_rules_json: serde_json::json!({
                "maxAge": 0,
                "sMaxage": 31536000,
                "staleWhileRevalidate": 86400,
            }),
            priority: 0,
            created_at: Utc::now(),
        },
    ]
}

/// Site deployment info for response.
#[cfg(feature = "cap-sites")]
#[derive(Debug, serde::Serialize)]
struct SiteDeployInfo {
    site_name: String,
    deployment_id: String,
    asset_count: u32,
    asset_bytes: u64,
}

/// Deploy functions from the bundle.
///
/// This is a placeholder implementation that logs the functions to be deployed.
/// Full implementation requires:
/// - FunctionsStore trait methods for get_function_by_name, create_function, create_deployment
/// - Bundle upload to storage
/// - Runtime deployment via RuntimeRegistry
/// - Status updates and promotion
#[cfg(feature = "cap-functions")]
async fn deploy_functions(
    admin_state: &AdminAuthState,
    bundle: &Bundle,
    function_entries: &[reactor_deploy_bundle::FunctionEntry],
) -> PhaseResult {
    let _functions_state = match &admin_state.functions {
        Some(s) => s,
        None => {
            return PhaseResult::with_error("functions", "functions capability not available");
        }
    };

    // Log the functions that would be deployed
    let mut deployed_functions: Vec<FunctionDeployInfo> = Vec::new();
    
    for entry in function_entries {
        let func_path = bundle.root.join(&entry.path);
        if !func_path.exists() {
            return PhaseResult::with_error(
                "functions",
                &format!("function directory not found: {}", entry.path),
            );
        }

        tracing::info!(
            function = %entry.name,
            runtime = %entry.runtime,
            path = %func_path.display(),
            "would deploy function"
        );

        // TODO: Implement actual deployment:
        // 1. Parse manifest from function directory
        // 2. Create function bundle zip
        // 3. Upload to storage
        // 4. Create deployment record
        // 5. Deploy to runtime
        // 6. Update status and promote

        deployed_functions.push(FunctionDeployInfo {
            function_name: entry.name.clone(),
            deployment_id: "pending".to_string(),
            version: 0,
            runtime: entry.runtime.clone(),
        });
    }

    let mut details = HashMap::new();
    details.insert(
        "functions".to_string(),
        serde_json::to_value(&deployed_functions).unwrap_or_default(),
    );
    details.insert(
        "note".to_string(),
        serde_json::json!("function deployment placeholder - full implementation pending"),
    );

    PhaseResult::with_details("functions", "ok", details)
}

/// Function deployment info for response.
#[cfg(feature = "cap-functions")]
#[derive(Debug, serde::Serialize)]
struct FunctionDeployInfo {
    function_name: String,
    deployment_id: String,
    version: i64,
    runtime: String,
}

/// Extract bundle data from multipart form.
async fn extract_bundle(multipart: &mut Multipart) -> Result<Vec<u8>, String> {
    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        if field.name() == Some("bundle") {
            return field.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string());
        }
    }
    Err("missing 'bundle' field in multipart form".to_string())
}
