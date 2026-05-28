//! Static site framework adapter.

use super::{BuildOpts, FrameworkAdapter, SiteBundle};
use crate::bundle::{BundleRoute, Manifest, RouteKind, CacheRules};
use crate::error::SitesError;
use crate::Framework;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

/// Static site adapter.
pub struct StaticAdapter;

impl StaticAdapter {
    /// Create a new static adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for StaticAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FrameworkAdapter for StaticAdapter {
    fn name(&self) -> Framework {
        Framework::Static
    }

    fn detect(&self, project_dir: &Path) -> bool {
        let package_json = project_dir.join("package.json");

        if !package_json.exists() {
            return project_dir.join("index.html").exists()
                || has_web_files(project_dir);
        }

        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                let has_build = pkg
                    .get("scripts")
                    .and_then(|s| s.get("build"))
                    .is_some();
                return !has_build;
            }
        }

        false
    }

    async fn build(&self, project_dir: &Path, opts: &BuildOpts) -> Result<SiteBundle, SitesError> {
        let output_dir = &opts.output_dir;
        let static_dir = output_dir.join("static");

        std::fs::create_dir_all(&static_dir)
            .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;

        copy_dir_recursive(project_dir, &static_dir)?;

        let site_name = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("static-site")
            .to_string();

        let manifest = Manifest {
            name: site_name,
            version: 0,
            framework: Framework::Static,
            routes: vec![
                // Root path explicitly maps to index.html
                BundleRoute {
                    pattern: "/".to_string(),
                    kind: RouteKind::Static,
                    target: "index.html".to_string(),
                    methods: None,
                    cache: Some(CacheRules {
                        max_age: Some(0),
                        s_maxage: None,
                        stale_while_revalidate: None,
                        immutable: false,
                    }),
                    fallback: None,
                    revalidate: None,
                    tags: vec![],
                },
                // Catch-all for other paths
                BundleRoute {
                    pattern: "/:path*".to_string(),
                    kind: RouteKind::Static,
                    target: "$path".to_string(),
                    methods: None,
                    cache: Some(CacheRules {
                        max_age: Some(0),
                        s_maxage: None,
                        stale_while_revalidate: None,
                        immutable: false,
                    }),
                    fallback: None,
                    revalidate: None,
                    tags: vec![],
                },
            ],
            functions: HashMap::new(),
            redirects: vec![],
            headers: vec![],
            env_keys: vec![],
            secret_keys: vec![],
            analytics: None,
        };

        Ok(SiteBundle {
            manifest,
            static_dir,
            functions: vec![],
            prerender: None,
        })
    }
}

fn has_web_files(dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    match ext.to_str() {
                        Some("html" | "htm" | "css" | "js") => return true,
                        _ => {}
                    }
                }
            }
        }
    }
    false
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SitesError> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)
            .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;

        for entry in std::fs::read_dir(src)
            .map_err(|e| SitesError::BundleInvalid(e.to_string()))?
        {
            let entry = entry.map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
            let path = entry.path();
            let dest_path = dst.join(entry.file_name());

            if path.is_dir() {
                if path.file_name() != Some(std::ffi::OsStr::new("node_modules"))
                    && path.file_name() != Some(std::ffi::OsStr::new(".git"))
                {
                    copy_dir_recursive(&path, &dest_path)?;
                }
            } else {
                std::fs::copy(&path, &dest_path)
                    .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
            }
        }
    }

    Ok(())
}
