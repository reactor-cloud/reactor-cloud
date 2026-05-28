//! Framework adapters for building site bundles.

mod detect;

#[cfg(feature = "framework-static")]
pub mod static_site;

#[cfg(feature = "framework-hono")]
pub mod hono;

#[cfg(feature = "framework-nextjs")]
pub mod nextjs;

pub use detect::detect_framework;

use crate::bundle::{Manifest, FunctionConfig};
use crate::error::SitesError;
use crate::Framework;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Build options for framework adapters.
#[derive(Debug, Clone)]
pub struct BuildOpts {
    /// Output directory for the bundle.
    pub output_dir: PathBuf,
    /// Environment variables to pass to the build.
    pub env: HashMap<String, String>,
    /// Node.js version to use (if applicable).
    pub node_version: Option<String>,
}

/// Site bundle produced by a framework adapter.
#[derive(Debug)]
pub struct SiteBundle {
    /// Bundle manifest.
    pub manifest: Manifest,
    /// Directory containing static assets.
    pub static_dir: PathBuf,
    /// Function bundles (one per function in manifest).
    pub functions: Vec<FunctionBundle>,
    /// Optional directory containing prerendered HTML.
    pub prerender: Option<PathBuf>,
}

/// Function bundle within a site bundle.
#[derive(Debug)]
pub struct FunctionBundle {
    /// Function name (matches key in manifest.functions).
    pub name: String,
    /// Function configuration from manifest.
    pub config: FunctionConfig,
    /// Directory containing the function code.
    pub code_dir: PathBuf,
}

/// Framework adapter trait.
///
/// Implemented by each supported framework to build site bundles.
#[async_trait]
pub trait FrameworkAdapter: Send + Sync {
    /// Get the framework type.
    fn name(&self) -> Framework;

    /// Detect if this adapter can handle the given project.
    fn detect(&self, project_dir: &Path) -> bool;

    /// Build the project into a site bundle.
    async fn build(&self, project_dir: &Path, opts: &BuildOpts) -> Result<SiteBundle, SitesError>;
}
