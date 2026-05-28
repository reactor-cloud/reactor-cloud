use crate::error::{EvalError, Result};
use crate::test::Fixture;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Manages isolated git worktrees for test runs
pub struct WorktreeManager {
    base_path: PathBuf,
}

impl WorktreeManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Create a new isolated worktree for a test run
    pub async fn create(&self, run_id: &str, fixture: &Fixture, fixtures_path: &Path) -> Result<PathBuf> {
        let worktree_path = self.base_path.join(run_id);
        fs::create_dir_all(&worktree_path).await?;

        match fixture {
            Fixture::Empty => {
                // Initialize an empty git repo
                self.init_empty_repo(&worktree_path).await?;
            }
            Fixture::Git { repo, ref_ } => {
                // Clone from git
                self.clone_git_repo(&worktree_path, repo, ref_).await?;
            }
            Fixture::Tarball { path } => {
                // Extract tarball
                let tarball_path = fixtures_path.join(path);
                self.extract_tarball(&worktree_path, &tarball_path).await?;
                self.init_empty_repo(&worktree_path).await?;
            }
            Fixture::Template { path } => {
                // Copy template directory
                let template_path = fixtures_path.join(path);
                self.copy_template(&worktree_path, &template_path).await?;
                self.init_empty_repo(&worktree_path).await?;
            }
        }

        info!("Created worktree at {:?}", worktree_path);
        Ok(worktree_path)
    }

    async fn init_empty_repo(&self, path: &Path) -> Result<()> {
        // Check if already a git repo
        if path.join(".git").exists() {
            return Ok(());
        }

        let output = Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()?;

        if !output.status.success() {
            return Err(EvalError::Git(format!(
                "Failed to init git repo: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "foundry@reactor.cloud"])
            .current_dir(path)
            .output()?;

        Command::new("git")
            .args(["config", "user.name", "Reactor Foundry"])
            .current_dir(path)
            .output()?;

        Ok(())
    }

    async fn clone_git_repo(&self, path: &Path, repo: &str, ref_: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["clone", "--depth", "1", "--branch", ref_, repo])
            .arg(path)
            .output()?;

        if !output.status.success() {
            return Err(EvalError::Git(format!(
                "Failed to clone repo: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    async fn extract_tarball(&self, path: &Path, tarball: &Path) -> Result<()> {
        if !tarball.exists() {
            return Err(EvalError::FixtureNotFound(
                tarball.display().to_string(),
            ));
        }

        let output = Command::new("tar")
            .args(["-xzf", &tarball.display().to_string()])
            .current_dir(path)
            .output()?;

        if !output.status.success() {
            return Err(EvalError::Worktree(format!(
                "Failed to extract tarball: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    async fn copy_template(&self, path: &Path, template: &Path) -> Result<()> {
        if !template.exists() {
            return Err(EvalError::FixtureNotFound(
                template.display().to_string(),
            ));
        }

        // Use cp -r for recursive copy
        let output = Command::new("cp")
            .args(["-r", &format!("{}/*", template.display()), "."])
            .current_dir(path)
            .output();

        // cp might fail if empty, try with shell
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("cp -r {}/* . 2>/dev/null || true", template.display()))
            .current_dir(path)
            .output()?;

        Ok(())
    }

    /// Clean up a worktree after a successful run
    pub async fn cleanup(&self, run_id: &str) -> Result<()> {
        let worktree_path = self.base_path.join(run_id);
        if worktree_path.exists() {
            fs::remove_dir_all(&worktree_path).await?;
            debug!("Cleaned up worktree: {:?}", worktree_path);
        }
        Ok(())
    }

    /// List all existing worktrees
    pub async fn list(&self) -> Result<Vec<String>> {
        let mut runs = Vec::new();
        if !self.base_path.exists() {
            return Ok(runs);
        }

        let mut entries = fs::read_dir(&self.base_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    runs.push(name.to_string());
                }
            }
        }

        Ok(runs)
    }

    /// Generate a new run ID
    pub fn generate_run_id() -> String {
        format!("run-{}", Uuid::new_v4().simple())
    }
}
