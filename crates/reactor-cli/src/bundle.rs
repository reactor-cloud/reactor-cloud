//! Bundle creation for deployment.

use crate::error::{CliError, CliResult};
use crate::project::Project;
use reactor_deploy_bundle::{
    BundleManifest, CapabilitiesManifest, DataManifest, FunctionEntry, JobEntry, MigrationEntry,
    SiteEntry,
};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Default bundle output filename.
pub const DEFAULT_BUNDLE_NAME: &str = "deploy.tar.zst";

/// Build a deployment bundle from a project.
pub fn build_bundle(project: &Project, output_path: &Path) -> CliResult<BundleManifest> {
    // Create manifest
    let manifest = BundleManifest {
        project_id: project.manifest.project_id.clone(),
        reactor_version: crate::VERSION.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        capabilities: build_capabilities_manifest(project)?,
    };

    // Create tar archive
    let file = File::create(output_path)?;
    let encoder = zstd::stream::Encoder::new(file, 3)?;
    let mut tar = tar::Builder::new(encoder);

    // Add manifest
    let manifest_json = serde_json::to_vec_pretty(&manifest)?;
    add_bytes_to_tar(&mut tar, "manifest.json", &manifest_json)?;

    // Add data migrations
    if let Some(ref data) = manifest.capabilities.data {
        for migration in &data.migrations {
            let src_path = project.migrations_dir().join(&migration.name);
            if src_path.exists() {
                add_file_to_tar(&mut tar, &src_path, &migration.path)?;
            }
        }
    }

    // Add functions as .fnpkg.zip bundles
    if let Some(ref _functions) = manifest.capabilities.functions {
        // Create temp dir for function bundles
        let temp_bundles_dir = std::env::temp_dir().join(format!(
            "reactor-fn-bundles-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));
        std::fs::create_dir_all(&temp_bundles_dir)?;
        
        // Build function bundles
        let bundles = build_all_function_bundles(project, &temp_bundles_dir)?;
        
        // Add each bundle to the tar
        for (bundle_path, _manifest) in &bundles {
            let bundle_name = bundle_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown.fnpkg.zip");
            add_file_to_tar(&mut tar, bundle_path, &format!("functions/{}", bundle_name))?;
        }
        
        // Cleanup temp dir
        let _ = std::fs::remove_dir_all(&temp_bundles_dir);
    }

    // Add sites - build via framework adapter or copy raw
    // Use project.manifest.sites to access source paths, not the bundle manifest
    if manifest.capabilities.sites.is_some() {
        for site_config in &project.manifest.sites {
            // Use configured source if explicit, otherwise fall back to sites/{name}
            let source_rel = if site_config.source == "sites" {
                format!("sites/{}", site_config.name)
            } else {
                site_config.source.clone()
            };
            let site_source = project.resolve_path(&source_rel);
            if site_source.exists() {
                #[cfg(feature = "framework-build")]
                {
                    // Build site via framework adapter
                    build_and_add_site(
                        &mut tar,
                        &site_source,
                        &site_config.name,
                        &site_config.framework,
                    )?;
                }
                #[cfg(not(feature = "framework-build"))]
                {
                    // Just copy raw site directory
                    add_directory_to_tar(&mut tar, &site_source, &format!("sites/{}", site_config.name))?;
                }
            }
        }
    }

    // Finish tar
    let encoder = tar.into_inner()?;
    encoder.finish()?;

    Ok(manifest)
}

/// Build the capabilities manifest from project configuration.
fn build_capabilities_manifest(project: &Project) -> CliResult<CapabilitiesManifest> {
    let mut caps = CapabilitiesManifest {
        data: None,
        storage: None,
        functions: None,
        jobs: None,
        sites: None,
        connect: None,
    };

    // Data migrations
    let migrations_dir = project.migrations_dir();
    if migrations_dir.exists() {
        let migrations = collect_migrations(&migrations_dir)?;
        if !migrations.is_empty() {
            caps.data = Some(DataManifest { migrations });
        }
    }

    // Functions
    let functions_dir = project.functions_dir();
    if functions_dir.exists() {
        let functions = collect_functions(project, &functions_dir)?;
        if !functions.is_empty() {
            caps.functions = Some(functions);
        }
    }

    // Sites - check manifest first, then auto-discover from sites dir
    if !project.manifest.sites.is_empty() {
        // Sites explicitly configured in manifest
        let sites = collect_sites(project, &project.sites_dir())?;
        if !sites.is_empty() {
            caps.sites = Some(sites);
        }
    } else {
        // Auto-discover from sites directory
        let sites_dir = project.sites_dir();
        if sites_dir.exists() {
            let sites = collect_sites(project, &sites_dir)?;
            if !sites.is_empty() {
                caps.sites = Some(sites);
            }
        }
    }

    // Jobs from manifest
    if !project.manifest.jobs.is_empty() {
        let jobs: Vec<JobEntry> = project
            .manifest
            .jobs
            .iter()
            .map(|j| JobEntry {
                name: j.name.clone(),
                function_name: j.function.clone(),
                path: format!("jobs/{}.json", j.name),
                sha256: String::new(), // Would compute in real impl
            })
            .collect();
        if !jobs.is_empty() {
            caps.jobs = Some(jobs);
        }
    }

    Ok(caps)
}

/// Collect migration files from a directory.
fn collect_migrations(dir: &Path) -> CliResult<Vec<MigrationEntry>> {
    let mut migrations = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "sql").unwrap_or(false) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let content = std::fs::read(&path)?;
                let hash = hex::encode(Sha256::digest(&content));

                migrations.push(MigrationEntry {
                    name: name.to_string(),
                    path: format!("data/migrations/{}", name),
                    sha256: hash,
                });
            }
        }
    }

    migrations.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(migrations)
}

/// Collect functions from the functions directory.
fn collect_functions(project: &Project, dir: &Path) -> CliResult<Vec<FunctionEntry>> {
    let mut functions = Vec::new();

    // If we have config entries, use those
    if !project.manifest.functions.is_empty() {
        for fc in &project.manifest.functions {
            functions.push(FunctionEntry {
                name: fc.name.clone(),
                path: format!("functions/{}", fc.name),
                sha256: String::new(), // Would compute hash in real impl
                runtime: fc.runtime.clone(),
            });
        }
    } else {
        // Otherwise, auto-discover from directory
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    functions.push(FunctionEntry {
                        name: name.to_string(),
                        path: format!("functions/{}", name),
                        sha256: String::new(),
                        runtime: "wasm".to_string(),
                    });
                }
            }
        }
    }

    Ok(functions)
}

/// Collect sites from the sites directory.
fn collect_sites(project: &Project, dir: &Path) -> CliResult<Vec<SiteEntry>> {
    let mut sites = Vec::new();

    if !project.manifest.sites.is_empty() {
        for sc in &project.manifest.sites {
            sites.push(SiteEntry {
                name: sc.name.clone(),
                path: format!("sites/{}", sc.name),
                sha256: String::new(),
                framework: sc.framework.clone(),
            });
        }
    } else {
        // Auto-discover
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    sites.push(SiteEntry {
                        name: name.to_string(),
                        path: format!("sites/{}", name),
                        sha256: String::new(),
                        framework: "static".to_string(),
                    });
                }
            }
        }
    }

    Ok(sites)
}

/// Add a file from bytes to tar archive.
fn add_bytes_to_tar<W: Write>(
    tar: &mut tar::Builder<W>,
    path: &str,
    data: &[u8],
) -> CliResult<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, data)?;
    Ok(())
}

/// Add a single file to tar archive.
fn add_file_to_tar<W: Write>(
    tar: &mut tar::Builder<W>,
    src_path: &Path,
    tar_path: &str,
) -> CliResult<()> {
    let mut file = File::open(src_path)?;
    let metadata = file.metadata()?;

    let mut header = tar::Header::new_gnu();
    header.set_size(metadata.len());
    header.set_mode(0o644);
    header.set_cksum();

    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    tar.append_data(&mut header, tar_path, data.as_slice())?;
    Ok(())
}

/// Add a directory to tar archive recursively.
fn add_directory_to_tar<W: Write>(
    tar: &mut tar::Builder<W>,
    src_dir: &Path,
    tar_prefix: &str,
) -> CliResult<()> {
    for entry in WalkDir::new(src_dir) {
        let entry = entry.map_err(|e| CliError::BundleValidation(e.to_string()))?;
        let path = entry.path();

        // Skip hidden files and directories
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }

        let relative = path
            .strip_prefix(src_dir)
            .map_err(|_| CliError::BundleValidation("path prefix error".into()))?;

        let tar_path = PathBuf::from(tar_prefix).join(relative);

        if path.is_file() {
            let mut file = File::open(path)?;
            let metadata = file.metadata()?;

            let mut header = tar::Header::new_gnu();
            header.set_size(metadata.len());
            header.set_mode(0o644);
            header.set_cksum();

            let mut data = Vec::new();
            file.read_to_end(&mut data)?;

            tar.append_data(&mut header, &tar_path, data.as_slice())?;
        }
    }

    Ok(())
}

/// Read a bundle file and return its bytes.
pub fn read_bundle(path: &Path) -> CliResult<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(data)
}

/// Function manifest for .fnpkg.zip bundles.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionManifest {
    /// Function name.
    pub name: String,
    /// Version number (0 for first deploy, incremented by server).
    pub version: i64,
    /// Runtime type (bun, wasm, lambda).
    pub runtime: String,
    /// Entrypoint file (relative to bundle root).
    pub entrypoint: String,
    /// Resource limits.
    #[serde(default)]
    pub limits: FunctionLimits,
    /// Concurrency configuration.
    #[serde(default)]
    pub concurrency: FunctionConcurrency,
    /// Required environment variable keys.
    #[serde(default)]
    pub env_keys: Vec<String>,
    /// Required secret keys.
    #[serde(default)]
    pub secret_keys: Vec<String>,
    /// Whether to forward the Authorization header.
    #[serde(default)]
    pub forward_authorization: bool,
}

/// Resource limits for function.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FunctionLimits {
    /// Timeout in milliseconds (default: 30000).
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Memory limit in MB (default: 128).
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
    /// Maximum request body size in MB (default: 6).
    #[serde(default = "default_max_body_mb")]
    pub max_body_in_mb: u32,
    /// Maximum response body size in MB (default: 6).
    #[serde(default = "default_max_body_out_mb")]
    pub max_body_out_mb: u32,
}

/// Concurrency configuration for function.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FunctionConcurrency {
    /// Minimum instances to keep warm (default: 0).
    #[serde(default)]
    pub min_instances: u32,
    /// Maximum concurrent invocations (default: 100).
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u32,
}

fn default_timeout_ms() -> u64 { 30000 }
fn default_memory_mb() -> u32 { 128 }
fn default_max_body_mb() -> u32 { 6 }
fn default_max_body_out_mb() -> u32 { 6 }
fn default_max_concurrency() -> u32 { 100 }

/// Build a per-function .fnpkg.zip bundle.
///
/// The bundle structure is:
/// - manifest.json: FunctionManifest with function metadata
/// - code/: Directory containing the function code
///   - index.ts (or the entrypoint file)
///   - other source files
///
/// Returns the path to the created bundle and the manifest.
pub fn build_function_bundle(
    project: &Project,
    function_name: &str,
    output_dir: &Path,
) -> CliResult<(PathBuf, FunctionManifest)> {
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    // Find function config in project manifest
    let func_config = project
        .manifest
        .functions
        .iter()
        .find(|f| f.name == function_name)
        .ok_or_else(|| {
            CliError::FunctionNotFound(format!(
                "Function '{}' not found in reactor.toml",
                function_name
            ))
        })?;

    // Locate function source directory
    let func_dir = project.functions_dir().join(function_name);
    if !func_dir.exists() {
        return Err(CliError::FunctionNotFound(format!(
            "Function directory not found: {}",
            func_dir.display()
        )));
    }

    // Determine entrypoint
    let entrypoint = func_config
        .entry
        .clone()
        .unwrap_or_else(|| detect_entrypoint(&func_dir));

    // Create function manifest with defaults
    let manifest = FunctionManifest {
        name: function_name.to_string(),
        version: 0, // Server assigns version on deploy
        runtime: func_config.runtime.clone(),
        entrypoint: format!("code/{}", entrypoint),
        limits: FunctionLimits::default(),
        concurrency: FunctionConcurrency::default(),
        env_keys: Vec::new(),
        secret_keys: Vec::new(),
        forward_authorization: false,
    };

    // Create output directory if needed
    std::fs::create_dir_all(output_dir)?;

    // Create zip file
    let bundle_path = output_dir.join(format!("{}.fnpkg.zip", function_name));
    let file = File::create(&bundle_path)?;
    let mut zip = ZipWriter::new(file);

    // Write manifest.json
    let manifest_json = serde_json::to_vec_pretty(&manifest)?;
    zip.start_file("manifest.json", SimpleFileOptions::default())
        .map_err(|e| CliError::BundleValidation(format!("zip error: {}", e)))?;
    zip.write_all(&manifest_json)?;

    // Add all files from function directory under code/
    for entry in WalkDir::new(&func_dir) {
        let entry = entry.map_err(|e| CliError::BundleValidation(e.to_string()))?;
        let path = entry.path();

        // Skip hidden files and node_modules
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
        }

        let relative = path
            .strip_prefix(&func_dir)
            .map_err(|_| CliError::BundleValidation("path prefix error".into()))?;

        if path.is_file() {
            let zip_path = format!("code/{}", relative.display());
            zip.start_file(&zip_path, SimpleFileOptions::default())
                .map_err(|e| CliError::BundleValidation(format!("zip error: {}", e)))?;
            
            let mut file = File::open(path)?;
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;
            zip.write_all(&data)?;
        }
    }

    zip.finish()
        .map_err(|e| CliError::BundleValidation(format!("zip finish error: {}", e)))?;

    // Compute SHA256 of the bundle
    let bundle_content = std::fs::read(&bundle_path)?;
    let _sha256 = hex::encode(Sha256::digest(&bundle_content));

    tracing::info!(
        function = %function_name,
        path = %bundle_path.display(),
        size = bundle_content.len(),
        "built function bundle"
    );

    Ok((bundle_path, manifest))
}

/// Detect the entrypoint file for a function.
fn detect_entrypoint(func_dir: &Path) -> String {
    // Check common entrypoint filenames in order of preference
    let candidates = [
        "index.ts",
        "index.js",
        "main.ts",
        "main.js",
        "handler.ts",
        "handler.js",
    ];

    for candidate in candidates {
        if func_dir.join(candidate).exists() {
            return candidate.to_string();
        }
    }

    // Default to index.ts
    "index.ts".to_string()
}

/// Build function bundles for all functions in the project.
pub fn build_all_function_bundles(
    project: &Project,
    output_dir: &Path,
) -> CliResult<Vec<(PathBuf, FunctionManifest)>> {
    let mut bundles = Vec::new();

    for func_config in &project.manifest.functions {
        match build_function_bundle(project, &func_config.name, output_dir) {
            Ok(bundle) => bundles.push(bundle),
            Err(e) => {
                tracing::warn!(
                    function = %func_config.name,
                    error = %e,
                    "failed to build function bundle"
                );
                return Err(e);
            }
        }
    }

    Ok(bundles)
}

/// Build a site using the framework adapter and add it to the tar archive.
#[cfg(feature = "framework-build")]
fn build_and_add_site<W: Write>(
    tar: &mut tar::Builder<W>,
    site_source: &Path,
    site_name: &str,
    framework: &str,
) -> CliResult<()> {
    use reactor_sites::framework::{BuildOpts, FrameworkAdapter};
    use std::collections::HashMap;

    // Create temp directory for build output
    let temp_dir = std::env::temp_dir().join(format!(
        "reactor-site-build-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    let opts = BuildOpts {
        output_dir: temp_dir.clone(),
        env: HashMap::new(),
        node_version: None,
    };

    // Select adapter based on framework and build.
    //
    // `build_bundle` is a synchronous function, but we're called from inside
    // a tokio runtime (the deploy command is async). We can't just call
    // `Handle::current().block_on(...)` because tokio refuses to nest a
    // blocking wait inside the runtime that's driving the current task.
    // `block_in_place` tells tokio to move the current task off the worker
    // thread so we can safely block here while async work completes.
    let bundle = match framework {
        "nextjs" | "next" => {
            #[cfg(feature = "framework-build")]
            {
                use reactor_sites::framework::nextjs::NextjsAdapter;
                let adapter = NextjsAdapter::new();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(adapter.build(site_source, &opts))
                })
                .map_err(|e| CliError::BundleValidation(format!("Next.js build failed: {}", e)))?
            }
        }
        "hono" => {
            #[cfg(feature = "framework-build")]
            {
                use reactor_sites::framework::hono::HonoAdapter;
                let adapter = HonoAdapter::new();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(adapter.build(site_source, &opts))
                })
                .map_err(|e| CliError::BundleValidation(format!("Hono build failed: {}", e)))?
            }
        }
        "static" | _ => {
            #[cfg(feature = "framework-build")]
            {
                use reactor_sites::framework::static_site::StaticAdapter;
                let adapter = StaticAdapter::new();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(adapter.build(site_source, &opts))
                })
                .map_err(|e| CliError::BundleValidation(format!("Static build failed: {}", e)))?
            }
        }
    };

    // Add manifest
    let manifest_json = serde_json::to_vec_pretty(&bundle.manifest)?;
    add_bytes_to_tar(tar, &format!("sites/{}/manifest.json", site_name), &manifest_json)?;

    // Add static files
    if bundle.static_dir.exists() {
        add_directory_to_tar(tar, &bundle.static_dir, &format!("sites/{}/static", site_name))?;
    }

    // Add function bundles
    for func_bundle in &bundle.functions {
        let func_tar_path = format!("sites/{}/functions/{}.fn", site_name, func_bundle.name);
        add_directory_to_tar(tar, &func_bundle.code_dir, &func_tar_path)?;
    }

    // Add prerender cache if present
    if let Some(prerender_dir) = &bundle.prerender {
        if prerender_dir.exists() {
            add_directory_to_tar(tar, prerender_dir, &format!("sites/{}/prerender", site_name))?;
        }
    }

    // Cleanup temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(())
}
