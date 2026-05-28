use crate::error::{EvalError, Result};
use crate::loader::TestLoader;
use crate::replay::{Cassette, CassetteManager, ReplayMode};
use crate::scorer::{ScoreResult, Scorer};
use crate::test::{Test, TestId, TestLevel};
use crate::worktree::WorktreeManager;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

/// Configuration for a test run
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Levels to run
    pub levels: Vec<TestLevel>,
    /// Maximum concurrent test runs
    pub concurrency: usize,
    /// Replay mode
    pub replay_mode: ReplayMode,
    /// Random seed for stochastic tests
    pub seed: Option<String>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            levels: vec![TestLevel::L0, TestLevel::L1, TestLevel::L2],
            concurrency: 4,
            replay_mode: ReplayMode::Live,
            seed: None,
        }
    }
}

impl RunConfig {
    pub fn with_levels(mut self, levels: Vec<TestLevel>) -> Self {
        self.levels = levels;
        self
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency;
        self
    }

    pub fn with_replay_mode(mut self, mode: ReplayMode) -> Self {
        self.replay_mode = mode;
        self
    }

    pub fn with_seed(mut self, seed: impl Into<String>) -> Self {
        self.seed = Some(seed.into());
        self
    }

    pub fn smoke() -> Self {
        Self::default().with_levels(vec![TestLevel::L0, TestLevel::L1, TestLevel::L2])
    }

    pub fn standard() -> Self {
        Self::default().with_levels(vec![
            TestLevel::L0,
            TestLevel::L1,
            TestLevel::L2,
            TestLevel::L3,
            TestLevel::L4,
        ])
    }

    pub fn full() -> Self {
        Self::default().with_levels(vec![
            TestLevel::L0,
            TestLevel::L1,
            TestLevel::L2,
            TestLevel::L3,
            TestLevel::L4,
            TestLevel::L5,
            TestLevel::L6,
            TestLevel::L7,
        ])
    }
}

/// The outcome of a single test run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutcome {
    pub test_id: TestId,
    pub level: TestLevel,
    pub passed: bool,
    pub runs: u32,
    pub passes: u32,
    pub pass_rate: f64,
    pub scores: Vec<ScoreResult>,
    pub duration_ms: u64,
    pub started_at: DateTime<Utc>,
    pub worktree_path: Option<PathBuf>,
    pub error: Option<String>,
}

impl RunOutcome {
    pub fn failure_signature(&self) -> Option<String> {
        if self.passed {
            return None;
        }
        
        // Generate a signature from failed scorer messages
        let failed_scorers: Vec<_> = self.scores.iter()
            .filter(|s| !s.passed)
            .map(|s| format!("{}:{}", s.scorer_kind, s.message))
            .collect();
        
        if failed_scorers.is_empty() {
            self.error.clone()
        } else {
            Some(failed_scorers.join(";"))
        }
    }
}

/// Results of running a suite of tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteResult {
    pub config: RunConfigSummary,
    pub outcomes: Vec<RunOutcome>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfigSummary {
    pub levels: Vec<String>,
    pub concurrency: usize,
    pub replay_mode: String,
}

impl SuiteResult {
    pub fn pass_rate(&self) -> f64 {
        if self.outcomes.is_empty() {
            return 0.0;
        }
        let passed = self.outcomes.iter().filter(|o| o.passed).count();
        passed as f64 / self.outcomes.len() as f64
    }

    pub fn pass_rate_by_level(&self) -> HashMap<TestLevel, f64> {
        let mut by_level: HashMap<TestLevel, (usize, usize)> = HashMap::new();
        
        for outcome in &self.outcomes {
            let entry = by_level.entry(outcome.level).or_insert((0, 0));
            entry.1 += 1;
            if outcome.passed {
                entry.0 += 1;
            }
        }

        by_level
            .into_iter()
            .map(|(level, (passed, total))| {
                (level, if total == 0 { 0.0 } else { passed as f64 / total as f64 })
            })
            .collect()
    }

    pub fn failures(&self) -> Vec<&RunOutcome> {
        self.outcomes.iter().filter(|o| !o.passed).collect()
    }

    pub fn total(&self) -> usize {
        self.outcomes.len()
    }

    pub fn passed(&self) -> usize {
        self.outcomes.iter().filter(|o| o.passed).count()
    }

    pub fn failed(&self) -> usize {
        self.outcomes.iter().filter(|o| !o.passed).count()
    }
}

/// The test runner
pub struct Runner {
    loader: TestLoader,
    worktree_manager: WorktreeManager,
    cassette_manager: CassetteManager,
    foundry_path: PathBuf,
}

impl Runner {
    pub fn new(foundry_path: impl Into<PathBuf>) -> Self {
        let foundry_path = foundry_path.into();
        Self {
            loader: TestLoader::new(foundry_path.join("../eval-suite")),
            worktree_manager: WorktreeManager::new(foundry_path.join("runs")),
            cassette_manager: CassetteManager::new(foundry_path.join("replays")),
            foundry_path,
        }
    }

    pub fn with_suite_path(mut self, suite_path: impl Into<PathBuf>) -> Self {
        self.loader = TestLoader::new(suite_path.into());
        self
    }

    /// Run the test suite with the given configuration
    pub async fn run_suite(&self, config: &RunConfig) -> Result<SuiteResult> {
        let started_at = Utc::now();
        let start_instant = Instant::now();

        // Load tests for the requested levels
        let tests = self.loader.load_levels(&config.levels).await?;
        info!("Running {} tests at levels {:?}", tests.len(), config.levels);

        // Run tests with concurrency limit
        let semaphore = Arc::new(Semaphore::new(config.concurrency));
        let mut handles = Vec::new();

        for test in tests {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let runner = self.clone_for_task();
            let replay_mode = config.replay_mode;
            let seed = config.seed.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let handle = tokio::spawn(async move {
                let result = runner.run_test(&test, replay_mode, &seed).await;
                drop(permit);
                result
            });
            handles.push(handle);
        }

        // Collect results
        let mut outcomes = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(outcome)) => outcomes.push(outcome),
                Ok(Err(e)) => error!("Test failed with error: {}", e),
                Err(e) => error!("Task panicked: {}", e),
            }
        }

        let completed_at = Utc::now();
        let duration_ms = start_instant.elapsed().as_millis() as u64;

        Ok(SuiteResult {
            config: RunConfigSummary {
                levels: config.levels.iter().map(|l| l.as_str().to_string()).collect(),
                concurrency: config.concurrency,
                replay_mode: format!("{:?}", config.replay_mode),
            },
            outcomes,
            started_at,
            completed_at,
            duration_ms,
        })
    }

    /// Run a single test
    pub async fn run_test(
        &self,
        test: &Test,
        replay_mode: ReplayMode,
        seed: &str,
    ) -> Result<RunOutcome> {
        let started_at = Utc::now();
        let start_instant = Instant::now();

        info!("Running test {} (level {:?})", test.id, test.level);

        // Create worktree
        let run_id = WorktreeManager::generate_run_id();
        let worktree_path = self
            .worktree_manager
            .create(&run_id, &test.fixture, &self.loader.fixtures_path())
            .await?;

        // For stochastic tests, run multiple times
        let mut passes = 0u32;
        let mut all_scores = Vec::new();

        for run_num in 0..test.runs_required {
            debug!("Test {} run {}/{}", test.id, run_num + 1, test.runs_required);

            // TODO: Actually run the agent here
            // For now, just evaluate the scorers against the fixture state
            let mut run_passed = true;
            for scorer in &test.success {
                let result = Scorer::evaluate(scorer, &worktree_path).await?;
                if !result.passed {
                    run_passed = false;
                }
                if run_num == 0 {
                    all_scores.push(result);
                }
            }

            if run_passed {
                passes += 1;
            }
        }

        let pass_rate = passes as f64 / test.runs_required as f64;
        let passed = pass_rate >= test.pass_threshold;

        let duration_ms = start_instant.elapsed().as_millis() as u64;

        // Clean up worktree on success, retain on failure
        let worktree_to_return = if passed {
            self.worktree_manager.cleanup(&run_id).await?;
            None
        } else {
            Some(worktree_path)
        };

        Ok(RunOutcome {
            test_id: test.id.clone(),
            level: test.level,
            passed,
            runs: test.runs_required,
            passes,
            pass_rate,
            scores: all_scores,
            duration_ms,
            started_at,
            worktree_path: worktree_to_return,
            error: None,
        })
    }

    fn clone_for_task(&self) -> Self {
        Self {
            loader: TestLoader::new(self.loader.fixtures_path().parent().unwrap()),
            worktree_manager: WorktreeManager::new(self.foundry_path.join("runs")),
            cassette_manager: CassetteManager::new(self.foundry_path.join("replays")),
            foundry_path: self.foundry_path.clone(),
        }
    }
}
