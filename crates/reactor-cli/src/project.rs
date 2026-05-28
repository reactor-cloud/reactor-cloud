//! Project manifest handling (reactor.toml).

use crate::error::{CliError, CliResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Project manifest filename.
pub const MANIFEST_FILENAME: &str = "reactor.toml";

/// Ignore file name.
pub const IGNORE_FILENAME: &str = ".reactorignore";

/// Project manifest (reactor.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    /// Project ID (unique identifier).
    pub project_id: String,

    /// Human-readable project name.
    pub name: String,

    /// Default context to use for this project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_context: Option<String>,

    /// Functions configuration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<FunctionConfig>,

    /// Sites configuration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sites: Vec<SiteConfig>,

    /// Jobs configuration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<JobConfig>,

    /// Data configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<DataConfig>,
}

/// Function configuration in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionConfig {
    /// Function name.
    pub name: String,

    /// Source directory relative to project root.
    #[serde(default = "default_function_source")]
    pub source: String,

    /// Runtime type (wasm, bun, lambda).
    #[serde(default = "default_function_runtime")]
    pub runtime: String,

    /// Entry point file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

fn default_function_source() -> String {
    "functions".to_string()
}

fn default_function_runtime() -> String {
    "wasm".to_string()
}

/// Site configuration in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    /// Site name.
    pub name: String,

    /// Source directory relative to project root.
    #[serde(default = "default_site_source")]
    pub source: String,

    /// Framework (static, hono, nextjs).
    #[serde(default = "default_site_framework")]
    pub framework: String,
}

fn default_site_source() -> String {
    "sites".to_string()
}

fn default_site_framework() -> String {
    "static".to_string()
}

/// Job configuration in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    /// Job name.
    pub name: String,

    /// Function name to invoke.
    pub function: String,

    /// Trigger configuration.
    pub trigger: JobTriggerConfig,
}

/// Job trigger configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobTriggerConfig {
    /// Cron-based trigger.
    Cron {
        /// Cron expression.
        schedule: String,
    },
    /// Event-based trigger.
    Event {
        /// Event type to listen for.
        event_type: String,
    },
    /// Manual trigger only.
    Manual,
}

/// Data configuration in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    /// Migrations directory relative to project root.
    #[serde(default = "default_migrations_dir")]
    pub migrations_dir: String,
}

fn default_migrations_dir() -> String {
    "data/migrations".to_string()
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            migrations_dir: default_migrations_dir(),
        }
    }
}

/// A resolved project with its manifest and paths.
#[derive(Debug, Clone)]
pub struct Project {
    /// The project manifest.
    pub manifest: ProjectManifest,

    /// Path to the manifest file.
    pub manifest_path: PathBuf,

    /// Project root directory (parent of manifest file).
    pub root: PathBuf,
}

impl Project {
    /// Resolve a project by walking up from the given directory.
    ///
    /// If `manifest_path` is provided, use that directly.
    /// Otherwise, walk up from `start_dir` looking for `reactor.toml`.
    pub fn resolve(start_dir: &Path, manifest_path: Option<&Path>) -> CliResult<Self> {
        let manifest_path = match manifest_path {
            Some(p) => {
                if !p.exists() {
                    return Err(CliError::ManifestNotFound);
                }
                p.to_path_buf()
            }
            None => find_manifest(start_dir)?,
        };

        let root = manifest_path
            .parent()
            .ok_or_else(|| CliError::InvalidManifest("manifest has no parent directory".into()))?
            .to_path_buf();

        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest: ProjectManifest = toml_edit::de::from_str(&content)
            .map_err(|e| CliError::InvalidManifest(e.to_string()))?;

        Ok(Self {
            manifest,
            manifest_path,
            root,
        })
    }

    /// Try to resolve a project, returning None if not found.
    pub fn try_resolve(start_dir: &Path, manifest_path: Option<&Path>) -> Option<Self> {
        Self::resolve(start_dir, manifest_path).ok()
    }

    /// Get the absolute path to a project-relative path.
    pub fn resolve_path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// Get the migrations directory path.
    pub fn migrations_dir(&self) -> PathBuf {
        let dir = self
            .manifest
            .data
            .as_ref()
            .map(|d| d.migrations_dir.as_str())
            .unwrap_or("data/migrations");
        self.resolve_path(dir)
    }

    /// Get the functions directory path.
    pub fn functions_dir(&self) -> PathBuf {
        self.resolve_path("functions")
    }

    /// Get the sites directory path.
    pub fn sites_dir(&self) -> PathBuf {
        self.resolve_path("sites")
    }
}

/// Find the manifest file by walking up from the given directory.
fn find_manifest(start_dir: &Path) -> CliResult<PathBuf> {
    let mut current = start_dir.to_path_buf();

    loop {
        let manifest_path = current.join(MANIFEST_FILENAME);
        if manifest_path.exists() {
            return Ok(manifest_path);
        }

        // Stop at git root
        if current.join(".git").exists() {
            break;
        }

        // Move to parent
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    Err(CliError::ManifestNotFound)
}

/// Generate a new project manifest.
pub fn generate_manifest(name: &str) -> ProjectManifest {
    let project_id = format!(
        "{}-{}",
        name.to_lowercase().replace(' ', "-"),
        &uuid::Uuid::now_v7().to_string()[..8]
    );

    ProjectManifest {
        project_id,
        name: name.to_string(),
        default_context: Some("local".to_string()),
        functions: vec![],
        sites: vec![],
        jobs: vec![],
        data: Some(DataConfig::default()),
    }
}

/// Generate the default .reactorignore content.
pub fn generate_ignore() -> &'static str {
    r#"# Reactor ignore file
# Files and directories matching these patterns will be excluded from bundles.

# Build artifacts
target/
dist/
build/
.next/
node_modules/

# IDE and editor
.idea/
.vscode/
*.swp
*.swo
*~

# OS files
.DS_Store
Thumbs.db

# Environment and secrets
.env
.env.*
*.pem
*.key

# Logs
*.log
logs/

# Test and coverage
coverage/
.nyc_output/

# Temporary files
tmp/
temp/
*.tmp
"#
}

/// Generate a sample function.
pub fn generate_sample_function() -> &'static str {
    r#"// Sample Reactor function
// This function is invoked via HTTP requests to /fn/v1/hello

export async function handler(request: Request): Promise<Response> {
  const body = await request.json().catch(() => ({}));
  const name = body.name || 'World';

  return new Response(
    JSON.stringify({ message: `Hello, ${name}!` }),
    {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    }
  );
}
"#
}

/// Generate a sample migration.
pub fn generate_sample_migration() -> &'static str {
    r#"-- Sample migration
-- This creates a simple 'items' table

CREATE TABLE IF NOT EXISTS items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add RLS policy (uncomment to enable)
-- ALTER TABLE items ENABLE ROW LEVEL SECURITY;
-- CREATE POLICY items_org_policy ON items
--     USING (org_id = current_setting('reactor.org_id')::uuid);
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_manifest() {
        let manifest = generate_manifest("My Project");
        assert_eq!(manifest.name, "My Project");
        assert!(manifest.project_id.starts_with("my-project-"));
        assert_eq!(manifest.default_context, Some("local".to_string()));
    }

    #[test]
    fn test_serialize_manifest() {
        let manifest = generate_manifest("test");
        let toml = toml_edit::ser::to_string_pretty(&manifest).unwrap();
        assert!(toml.contains("project_id"));
        assert!(toml.contains("name"));
    }
}
