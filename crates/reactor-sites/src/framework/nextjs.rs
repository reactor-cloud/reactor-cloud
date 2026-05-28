//! Next.js framework adapter.

use super::{BuildOpts, FrameworkAdapter, FunctionBundle, SiteBundle};
use crate::bundle::{BundleRoute, FunctionConfig, FunctionLimits, Manifest, RouteKind, CacheRules};
use crate::error::SitesError;
use crate::Framework;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Next.js framework adapter.
pub struct NextjsAdapter;

impl NextjsAdapter {
    /// Create a new Next.js adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NextjsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FrameworkAdapter for NextjsAdapter {
    fn name(&self) -> Framework {
        Framework::Nextjs
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

                return deps.map(|d| d.contains_key("next")).unwrap_or(false)
                    || dev_deps.map(|d| d.contains_key("next")).unwrap_or(false);
            }
        }

        false
    }

    async fn build(&self, project_dir: &Path, opts: &BuildOpts) -> Result<SiteBundle, SitesError> {
        // Check for output mode configuration
        let output_mode = detect_output_mode(project_dir)?;

        let output_dir = &opts.output_dir;
        let static_dir = output_dir.join("static");
        let functions_dir = output_dir.join("functions");
        let prerender_dir = output_dir.join("prerender");

        std::fs::create_dir_all(&static_dir)
            .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
        std::fs::create_dir_all(&functions_dir)
            .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;

        let mut env = opts.env.clone();
        env.insert("NEXT_TELEMETRY_DISABLED".to_string(), "1".to_string());

        let (pm_cmd, pm_args) = detect_package_manager(project_dir);

        let status = Command::new(pm_cmd)
            .current_dir(project_dir)
            .args(pm_args)
            .envs(&env)
            .status()
            .map_err(|e| SitesError::BundleInvalid(format!("failed to run {}: {}", pm_cmd, e)))?;

        if !status.success() {
            return Err(SitesError::BundleInvalid(format!("{} build failed", pm_cmd)));
        }

        let site_name = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("nextjs-app")
            .to_string();

        // Handle 'export' mode - purely static site
        if output_mode == OutputMode::Export {
            // Copy exported files from out/ directory
            let out_dir = project_dir.join("out");
            if !out_dir.exists() {
                return Err(SitesError::BundleInvalid(
                    "out/ directory not found. Make sure 'next build' runs with output: 'export'".to_string()
                ));
            }
            copy_dir_recursive(&out_dir, &static_dir)?;

            // For static export, all routes serve static files
            let routes = vec![
                BundleRoute {
                    pattern: "/_next/static/:path*".to_string(),
                    kind: RouteKind::Static,
                    target: "_next/static/$path".to_string(),
                    methods: None,
                    cache: Some(CacheRules {
                        max_age: Some(31536000),
                        s_maxage: None,
                        stale_while_revalidate: None,
                        immutable: true,
                    }),
                    fallback: None,
                    revalidate: None,
                    tags: vec![],
                },
                // Explicit root route - matchit's {*path} catch-all doesn't match /
                BundleRoute {
                    pattern: "/".to_string(),
                    kind: RouteKind::Static,
                    target: "index.html".to_string(),
                    methods: None,
                    cache: Some(CacheRules {
                        max_age: Some(0),
                        s_maxage: Some(31536000),
                        stale_while_revalidate: Some(86400),
                        immutable: false,
                    }),
                    fallback: None,
                    revalidate: None,
                    tags: vec![],
                },
                // Catch-all route for other static HTML files
                BundleRoute {
                    pattern: "/:path*".to_string(),
                    kind: RouteKind::Static,
                    target: "$path".to_string(),
                    methods: None,
                    cache: Some(CacheRules {
                        max_age: Some(0),
                        s_maxage: Some(31536000),
                        stale_while_revalidate: Some(86400),
                        immutable: false,
                    }),
                    fallback: None,
                    revalidate: None,
                    tags: vec![],
                },
            ];

            let manifest = Manifest {
                name: site_name,
                version: 0,
                framework: Framework::Nextjs,
                routes,
                functions: HashMap::new(),
                redirects: vec![],
                headers: vec![],
                env_keys: vec![],
                secret_keys: vec![],
                analytics: None,
            };

            return Ok(SiteBundle {
                manifest,
                static_dir,
                functions: vec![],
                prerender: None,
            });
        }

        // Handle 'standalone' mode - SSR with functions
        let next_static = project_dir.join(".next/static");
        if next_static.exists() {
            let dest = static_dir.join("_next/static");
            std::fs::create_dir_all(&dest)
                .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
            copy_dir_recursive(&next_static, &dest)?;
        }

        let public_dir = project_dir.join("public");
        if public_dir.exists() {
            copy_dir_recursive(&public_dir, &static_dir)?;
        }

        let standalone_dir = project_dir.join(".next/standalone");
        let mut functions = HashMap::new();
        let mut function_bundles = vec![];

        if standalone_dir.exists() {
            // The function bundle structure should be:
            //   ssr.fn/
            //     code/
            //       index.ts (our shim)
            //       standalone/ (Next.js standalone output)
            // The bundle.rs adds the contents of FunctionBundle.code_dir to the tar,
            // so we use ssr_dir as the code_dir to include the `code/` directory itself.
            let ssr_dir = functions_dir.join("ssr.fn");
            let code_dir = ssr_dir.join("code");
            std::fs::create_dir_all(&code_dir)
                .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;

            // Copy standalone output to code/standalone/
            let standalone_dest = code_dir.join("standalone");
            std::fs::create_dir_all(&standalone_dest)
                .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
            copy_dir_recursive(&standalone_dir, &standalone_dest)?;

            // Copy .next/static to code/standalone/.next/static (needed for SSR)
            let next_static_src = project_dir.join(".next/static");
            let next_static_dest = standalone_dest.join(".next/static");
            if next_static_src.exists() {
                std::fs::create_dir_all(&next_static_dest)
                    .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
                copy_dir_recursive(&next_static_src, &next_static_dest)?;
            }

            // Generate the Bun-compatible fetch handler shim
            let shim_content = generate_nextjs_bun_shim();
            let shim_path = code_dir.join("index.ts");
            std::fs::write(&shim_path, shim_content)
                .map_err(|e| SitesError::BundleInvalid(format!("failed to write index.ts shim: {}", e)))?;

            functions.insert(
                "ssr".to_string(),
                FunctionConfig {
                    runtime: "bun".to_string(),
                    entrypoint: "code/index.ts".to_string(),
                    limits: FunctionLimits {
                        timeout_ms: 30_000,
                        memory_mb: 512,
                    },
                },
            );

            // Use ssr_dir (not code_dir) as the bundle directory so the `code/`
            // subdirectory is included in the tar archive.
            function_bundles.push(FunctionBundle {
                name: "ssr".to_string(),
                config: FunctionConfig {
                    runtime: "bun".to_string(),
                    entrypoint: "code/index.ts".to_string(),
                    limits: FunctionLimits {
                        timeout_ms: 30_000,
                        memory_mb: 512,
                    },
                },
                code_dir: ssr_dir,
            });
        }

        let app_dir = project_dir.join(".next/server/app");
        let has_api = project_dir.join("app/api").exists()
            || project_dir.join("src/app/api").exists();

        if has_api && !functions.contains_key("api") {
            functions.insert(
                "api".to_string(),
                FunctionConfig {
                    runtime: "bun".to_string(),
                    entrypoint: "code/server.js".to_string(),
                    limits: FunctionLimits {
                        timeout_ms: 30_000,
                        memory_mb: 256,
                    },
                },
            );
        }

        let mut prerender_paths = vec![];
        if app_dir.exists() {
            std::fs::create_dir_all(&prerender_dir)
                .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
            collect_prerendered(&app_dir, &prerender_dir, "", &mut prerender_paths)?;
        }

        let mut routes = vec![];

        routes.push(BundleRoute {
            pattern: "/_next/static/:path*".to_string(),
            kind: RouteKind::Static,
            target: "_next/static/$path".to_string(),
            methods: None,
            cache: Some(CacheRules {
                max_age: Some(31536000),
                s_maxage: None,
                stale_while_revalidate: None,
                immutable: true,
            }),
            fallback: None,
            revalidate: None,
            tags: vec![],
        });

        if has_api {
            routes.push(BundleRoute {
                pattern: "/api/:path*".to_string(),
                kind: RouteKind::Function,
                target: if functions.contains_key("api") {
                    "api"
                } else {
                    "ssr"
                }
                .to_string(),
                methods: None,
                cache: None,
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
            framework: Framework::Nextjs,
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
            functions: function_bundles,
            prerender: if prerender_paths.is_empty() {
                None
            } else {
                Some(prerender_dir)
            },
        })
    }
}

/// Output mode for Next.js builds
#[derive(Debug, Clone, PartialEq)]
enum OutputMode {
    /// Static export (output: 'export') - generates static HTML files
    Export,
    /// Standalone server (output: 'standalone') - generates Node.js server for SSR  
    Standalone,
    /// Default Next.js behavior (no output config)
    Default,
}

/// Generate a Bun-compatible fetch handler shim for Next.js standalone output.
///
/// This shim wraps the Next.js standalone server.js in a way that's compatible
/// with Reactor's Bun function runtime, which expects:
///   export default { fetch(req: Request): Response | Promise<Response> }
///
/// The approach: run the Next.js server on localhost and proxy requests to it.
/// This is more reliable than trying to adapt Node's req/res to Fetch API directly.
fn generate_nextjs_bun_shim() -> String {
    r#"/**
 * Next.js Standalone → Bun Fetch Adapter
 * 
 * This shim starts the Next.js standalone server on an ephemeral port
 * and proxies Fetch API requests to it.
 */

import { spawn, type Subprocess } from "bun";
import { join, dirname } from "path";

// Path to the standalone server.js
const STANDALONE_DIR = join(dirname(import.meta.path), "standalone");
const SERVER_JS = join(STANDALONE_DIR, "server.js");

// Find an available port
async function findPort(): Promise<number> {
  const server = Bun.serve({
    port: 0,
    fetch: () => new Response(""),
  });
  const port = server.port;
  server.stop(true);
  return port;
}

let nextProcess: Subprocess | null = null;
let nextPort: number | null = null;
let startPromise: Promise<void> | null = null;

// Start the Next.js server if not already running
async function ensureServerRunning(): Promise<number> {
  if (nextPort !== null) {
    return nextPort;
  }

  if (startPromise) {
    await startPromise;
    return nextPort!;
  }

  startPromise = (async () => {
    const port = await findPort();
    
    console.log(`[nextjs-shim] Starting Next.js server on port ${port}`);

    nextProcess = spawn({
      cmd: ["node", SERVER_JS],
      cwd: STANDALONE_DIR,
      env: {
        ...process.env,
        PORT: String(port),
        HOSTNAME: "127.0.0.1",
        NODE_ENV: "production",
      },
      stdout: "inherit",
      stderr: "inherit",
    });

    // Wait for server to be ready
    const maxAttempts = 50;
    for (let i = 0; i < maxAttempts; i++) {
      try {
        const res = await fetch(`http://127.0.0.1:${port}/`, {
          method: "HEAD",
        });
        if (res.ok || res.status === 404) {
          console.log(`[nextjs-shim] Next.js server ready on port ${port}`);
          nextPort = port;
          return;
        }
      } catch {
        // Server not ready yet
      }
      await new Promise((r) => setTimeout(r, 100));
    }

    throw new Error("Next.js server failed to start within 5 seconds");
  })();

  await startPromise;
  return nextPort!;
}

// Handle process exit
process.on("SIGTERM", () => {
  if (nextProcess) {
    nextProcess.kill();
  }
});

process.on("SIGINT", () => {
  if (nextProcess) {
    nextProcess.kill();
  }
});

// Export the fetch handler
export default {
  async fetch(req: Request): Promise<Response> {
    const port = await ensureServerRunning();

    // Parse the incoming URL
    const url = new URL(req.url);

    // Forward to the Next.js server
    const targetUrl = `http://127.0.0.1:${port}${url.pathname}${url.search}`;

    // Create forwarded request
    const forwardHeaders = new Headers(req.headers);
    // Remove host header to avoid issues
    forwardHeaders.delete("host");
    // Add original host for Next.js if needed
    forwardHeaders.set("x-forwarded-host", url.host);
    forwardHeaders.set("x-forwarded-proto", url.protocol.replace(":", ""));

    const forwardReq = new Request(targetUrl, {
      method: req.method,
      headers: forwardHeaders,
      body: req.body,
      // @ts-ignore - duplex required for streaming body
      duplex: "half",
    });

    try {
      const response = await fetch(forwardReq);

      // Clone response headers to make them mutable
      const responseHeaders = new Headers(response.headers);
      
      // Return the response
      return new Response(response.body, {
        status: response.status,
        statusText: response.statusText,
        headers: responseHeaders,
      });
    } catch (err) {
      console.error("[nextjs-shim] Proxy error:", err);
      return new Response(
        JSON.stringify({ error: "Failed to proxy to Next.js server" }),
        {
          status: 502,
          headers: { "content-type": "application/json" },
        }
      );
    }
  },
};
"#.to_string()
}

/// Detect the package manager based on lockfiles in the project directory.
/// Returns (command, args) tuple for running the build script.
fn detect_package_manager(project_dir: &Path) -> (&'static str, &'static [&'static str]) {
    // Check for bun lockfile first (fastest runtime)
    if project_dir.join("bun.lockb").exists() || project_dir.join("bun.lock").exists() {
        return ("bun", &["run", "build"]);
    }
    // Check for pnpm
    if project_dir.join("pnpm-lock.yaml").exists() {
        return ("pnpm", &["run", "build"]);
    }
    // Check for yarn
    if project_dir.join("yarn.lock").exists() {
        return ("yarn", &["build"]);
    }
    // Default to npm
    ("npm", &["run", "build"])
}

/// Detect the output mode from Next.js config.
fn detect_output_mode(project_dir: &Path) -> Result<OutputMode, SitesError> {
    let config_files = [
        "next.config.ts",
        "next.config.js",
        "next.config.mjs",
    ];

    for config_file in config_files {
        let config_path = project_dir.join(config_file);
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| SitesError::BundleInvalid(format!("failed to read {}: {}", config_file, e)))?;
            
            // Check for output mode - look for output: "standalone" or output: "export"
            // Be careful: "export default" is not the same as output: "export"
            // Check for standalone first since it's unambiguous
            if content.contains("output") {
                // Check for various quote styles: "standalone", 'standalone'
                if content.contains(r#""standalone""#) || content.contains("'standalone'") {
                    return Ok(OutputMode::Standalone);
                }
                // Check for output: "export" (not "export default")
                if content.contains(r#""export""#) || content.contains("'export'") {
                    return Ok(OutputMode::Export);
                }
            }
        }
    }

    // Default mode requires 'standalone' for SSR deployment
    Err(SitesError::BundleInvalid(
        "Next.js project must have output: 'standalone' or output: 'export' in next.config.ts/js/mjs. \n\
        For static sites, add:  output: 'export'\n\
        For SSR sites, add:     output: 'standalone'".to_string()
    ))
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

fn collect_prerendered(
    src: &Path,
    dst: &Path,
    prefix: &str,
    paths: &mut Vec<String>,
) -> Result<(), SitesError> {
    if !src.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(src)
        .map_err(|e| SitesError::BundleInvalid(e.to_string()))?
    {
        let entry = entry.map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            let new_prefix = if prefix.is_empty() {
                format!("/{}", name_str)
            } else {
                format!("{}/{}", prefix, name_str)
            };
            collect_prerendered(&path, &dst.join(&*name_str), &new_prefix, paths)?;
        } else if name_str.ends_with(".html") {
            let dest_dir = dst;
            std::fs::create_dir_all(dest_dir)
                .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;
            std::fs::copy(&path, dest_dir.join(&*name_str))
                .map_err(|e| SitesError::BundleInvalid(e.to_string()))?;

            let route_path = if name_str == "page.html" {
                prefix.to_string()
            } else {
                let without_ext = name_str.strip_suffix(".html").unwrap_or(&name_str);
                if prefix.is_empty() {
                    format!("/{}", without_ext)
                } else {
                    format!("{}/{}", prefix, without_ext)
                }
            };

            if !route_path.is_empty() {
                paths.push(route_path);
            }
        }
    }

    Ok(())
}
