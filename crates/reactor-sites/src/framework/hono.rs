//! Hono framework adapter.

use super::{BuildOpts, FrameworkAdapter, FunctionBundle, SiteBundle};
use crate::bundle::{BundleRoute, FunctionConfig, FunctionLimits, Manifest, RouteKind, CacheRules};
use crate::error::SitesError;
use crate::Framework;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Hono framework adapter.
pub struct HonoAdapter;

impl HonoAdapter {
    /// Create a new Hono adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for HonoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FrameworkAdapter for HonoAdapter {
    fn name(&self) -> Framework {
        Framework::Hono
    }

    fn detect(&self, project_dir: &Path) -> bool {
        let package_json = project_dir.join("package.json");

        if !package_json.exists() {
            return false;
        }

        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                let deps = pkg.get("dependencies").and_then(|d| d.as_object());
                let dev_deps = pkg.get("devDependencies").and_then(|d| d.as_object());

                let has_hono = deps.map(|d| d.contains_key("hono")).unwrap_or(false)
                    || dev_deps.map(|d| d.contains_key("hono")).unwrap_or(false);

                if has_hono {
                    return project_dir.join("src/index.ts").exists()
                        || project_dir.join("index.ts").exists();
                }
            }
        }

        false
    }

    async fn build(&self, project_dir: &Path, opts: &BuildOpts) -> Result<SiteBundle, SitesError> {
        let output_dir = &opts.output_dir;
        let static_dir = output_dir.join("static");
        let functions_dir = output_dir.join("functions");
        let ssr_dir = functions_dir.join("ssr.fn");
        let code_dir = ssr_dir.join("code");

        std::fs::create_dir_all(&static_dir)
            .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
        std::fs::create_dir_all(&code_dir)
            .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;

        let public_dir = project_dir.join("public");
        if public_dir.exists() {
            copy_dir_recursive(&public_dir, &static_dir)?;
        }

        let entrypoint = if project_dir.join("src/index.ts").exists() {
            "src/index.ts"
        } else {
            "index.ts"
        };

        let status = Command::new("bun")
            .current_dir(project_dir)
            .args([
                "build",
                entrypoint,
                "--outdir",
                code_dir.to_str().unwrap(),
                "--target",
                "bun",
            ])
            .envs(&opts.env)
            .status()
            .map_err(|e| SitesError::BundleInvalid(format!("failed to run bun: {}", e)))?;

        if !status.success() {
            return Err(SitesError::BundleInvalid("bun build failed".to_string()));
        }

        let site_name = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("hono-app")
            .to_string();

        let mut functions = HashMap::new();
        functions.insert(
            "ssr".to_string(),
            FunctionConfig {
                runtime: "bun".to_string(),
                entrypoint: "code/index.js".to_string(),
                limits: FunctionLimits {
                    timeout_ms: 30_000,
                    memory_mb: 256,
                },
            },
        );

        let mut routes = vec![];

        if static_dir.exists() && std::fs::read_dir(&static_dir).map(|d| d.count()).unwrap_or(0) > 0
        {
            routes.push(BundleRoute {
                pattern: "/public/:path*".to_string(),
                kind: RouteKind::Static,
                target: "public/$path".to_string(),
                methods: None,
                cache: Some(CacheRules::default()),
                fallback: None,
                revalidate: None,
                tags: vec![],
            });
        }

        routes.push(BundleRoute {
            pattern: "/:path*".to_string(),
            kind: RouteKind::Function,
            target: "ssr".to_string(),
            methods: None,
            cache: None,
            fallback: None,
            revalidate: None,
            tags: vec![],
        });

        let manifest = Manifest {
            name: site_name,
            version: 0,
            framework: Framework::Hono,
            routes,
            functions,
            redirects: vec![],
            headers: vec![],
            env_keys: vec![],
            secret_keys: vec![],
            analytics: None,
        };

        Ok(SiteBundle {
            manifest,
            static_dir,
            functions: vec![FunctionBundle {
                name: "ssr".to_string(),
                config: FunctionConfig {
                    runtime: "bun".to_string(),
                    entrypoint: "code/index.js".to_string(),
                    limits: FunctionLimits::default(),
                },
                code_dir,
            }],
            prerender: None,
        })
    }
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
                copy_dir_recursive(&path, &dest_path)?;
            } else {
                std::fs::copy(&path, &dest_path)
                    .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
            }
        }
    }

    Ok(())
}
