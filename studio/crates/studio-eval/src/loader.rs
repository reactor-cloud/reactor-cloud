use crate::error::{EvalError, Result};
use crate::test::{Test, TestId, TestLevel};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

/// Loads tests from the eval-suite directory structure
pub struct TestLoader {
    suite_path: PathBuf,
}

impl TestLoader {
    pub fn new(suite_path: impl Into<PathBuf>) -> Self {
        Self {
            suite_path: suite_path.into(),
        }
    }

    /// Load all tests from the suite
    pub async fn load_all(&self) -> Result<Vec<Test>> {
        let mut tests = Vec::new();

        for level in [
            TestLevel::L0,
            TestLevel::L1,
            TestLevel::L2,
            TestLevel::L3,
            TestLevel::L4,
            TestLevel::L5,
            TestLevel::L6,
            TestLevel::L7,
        ] {
            tests.extend(self.load_level(level).await?);
        }

        info!("Loaded {} tests from {:?}", tests.len(), self.suite_path);
        Ok(tests)
    }

    /// Load tests for a specific level
    pub async fn load_level(&self, level: TestLevel) -> Result<Vec<Test>> {
        let level_dir = self.suite_path.join("tests").join(level.as_str());
        if !level_dir.exists() {
            debug!("Level directory does not exist: {:?}", level_dir);
            return Ok(Vec::new());
        }

        let mut tests = Vec::new();
        let mut entries = fs::read_dir(&level_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
                match self.load_test(&path).await {
                    Ok(test) => tests.push(test),
                    Err(e) => warn!("Failed to load test from {:?}: {}", path, e),
                }
            }
        }

        // Also check auto-minted subdirectory
        let auto_minted_dir = level_dir.join("auto-minted");
        if auto_minted_dir.exists() {
            let mut entries = fs::read_dir(&auto_minted_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
                    match self.load_test(&path).await {
                        Ok(test) => tests.push(test),
                        Err(e) => warn!("Failed to load auto-minted test from {:?}: {}", path, e),
                    }
                }
            }
        }

        debug!("Loaded {} tests for level {:?}", tests.len(), level);
        Ok(tests)
    }

    /// Load tests for multiple levels
    pub async fn load_levels(&self, levels: &[TestLevel]) -> Result<Vec<Test>> {
        let mut tests = Vec::new();
        for level in levels {
            tests.extend(self.load_level(*level).await?);
        }
        Ok(tests)
    }

    /// Load a single test from a YAML file
    pub async fn load_test(&self, path: &Path) -> Result<Test> {
        let content = fs::read_to_string(path).await?;
        let test: Test = serde_yaml::from_str(&content)?;
        debug!("Loaded test {} from {:?}", test.id, path);
        Ok(test)
    }

    /// Load a test by ID
    pub async fn load_by_id(&self, id: &TestId) -> Result<Test> {
        let tests = self.load_all().await?;
        tests
            .into_iter()
            .find(|t| &t.id == id)
            .ok_or_else(|| EvalError::TestNotFound(id.0.clone()))
    }

    /// Get test counts by level
    pub async fn count_by_level(&self) -> Result<HashMap<TestLevel, usize>> {
        let mut counts = HashMap::new();
        for level in [
            TestLevel::L0,
            TestLevel::L1,
            TestLevel::L2,
            TestLevel::L3,
            TestLevel::L4,
            TestLevel::L5,
            TestLevel::L6,
            TestLevel::L7,
        ] {
            counts.insert(level, self.load_level(level).await?.len());
        }
        Ok(counts)
    }

    /// Get the fixtures directory path
    pub fn fixtures_path(&self) -> PathBuf {
        self.suite_path.join("fixtures")
    }
}
