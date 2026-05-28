use crate::error::Result;
use crate::runner::{RunConfig, Runner, SuiteResult};
use crate::test::TestLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Events emitted by the auto-iteration loop
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopEvent {
    RunStarted {
        run_id: String,
        levels: Vec<String>,
        max_iterations: Option<u32>,
    },
    TestResult {
        run_id: String,
        test_id: String,
        passed: bool,
        scores: Vec<crate::scorer::ScoreResult>,
    },
    IterationComplete {
        iteration: u32,
        pass_rates: std::collections::HashMap<String, f64>,
        promotions: Vec<String>,
        demotions: Vec<String>,
    },
    LessonStaged {
        lesson_id: String,
        scope: String,
        kind: String,
    },
    LessonPromoted {
        lesson_id: String,
        from_tier: String,
        to_tier: String,
    },
    RunStopped {
        run_id: String,
        reason: String,
    },
    Error {
        location: String,
        message: String,
    },
}

/// Report for a single iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationReport {
    pub iteration: u32,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub pass_rate_before: f64,
    pub pass_rate_after: f64,
    pub failures_before: usize,
    pub failures_after: usize,
    pub lessons_staged: Vec<String>,
    pub lessons_promoted: Vec<(String, String, String)>, // (id, from, to)
    pub lessons_demoted: Vec<(String, String, String)>,
    pub lessons_rolled_back: Vec<String>,
    pub tests_auto_minted: Vec<String>,
}

/// Configuration for the auto-iteration loop
#[derive(Debug, Clone)]
pub struct LoopConfig {
    /// Test levels to run
    pub levels: Vec<TestLevel>,
    /// Maximum iterations (None = infinite)
    pub max_iterations: Option<u32>,
    /// Concurrency for test runs
    pub concurrency: usize,
    /// Whether to stop on first failure
    pub stop_on_failure: bool,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            levels: vec![TestLevel::L0, TestLevel::L1, TestLevel::L2],
            max_iterations: None,
            concurrency: 4,
            stop_on_failure: false,
        }
    }
}

/// State of the auto-iteration loop
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopState {
    Idle,
    Running,
    Stopping,
    Stopped,
}

/// The auto-iteration loop driver
pub struct AutoIterationLoop {
    runner: Runner,
    foundry_path: PathBuf,
    state: Arc<RwLock<LoopState>>,
    event_tx: broadcast::Sender<LoopEvent>,
}

impl AutoIterationLoop {
    pub fn new(foundry_path: impl Into<PathBuf>) -> Self {
        let foundry_path = foundry_path.into();
        let (event_tx, _) = broadcast::channel(1000);
        
        Self {
            runner: Runner::new(foundry_path.clone()),
            foundry_path,
            state: Arc::new(RwLock::new(LoopState::Idle)),
            event_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LoopEvent> {
        self.event_tx.subscribe()
    }

    pub async fn state(&self) -> LoopState {
        *self.state.read().await
    }

    /// Request the loop to stop after the current iteration
    pub async fn request_stop(&self) {
        let mut state = self.state.write().await;
        if *state == LoopState::Running {
            *state = LoopState::Stopping;
            info!("Stop requested for auto-iteration loop");
        }
    }

    /// Run the auto-iteration loop
    pub async fn run(&self, config: LoopConfig) -> Result<Vec<IterationReport>> {
        let run_id = uuid::Uuid::new_v4().to_string();
        
        {
            let mut state = self.state.write().await;
            if *state != LoopState::Idle {
                return Err(crate::error::EvalError::Runner(
                    "Loop is already running".to_string(),
                ));
            }
            *state = LoopState::Running;
        }

        self.emit(LoopEvent::RunStarted {
            run_id: run_id.clone(),
            levels: config.levels.iter().map(|l| l.as_str().to_string()).collect(),
            max_iterations: config.max_iterations,
        });

        let mut reports = Vec::new();
        let mut iteration = 0u32;

        loop {
            // Check for stop request
            {
                let state = self.state.read().await;
                if *state == LoopState::Stopping {
                    break;
                }
            }

            // Check iteration limit
            if let Some(max) = config.max_iterations {
                if iteration >= max {
                    break;
                }
            }

            iteration += 1;
            info!("Starting iteration {}", iteration);

            let report = match self.run_iteration(&config, iteration).await {
                Ok(report) => report,
                Err(e) => {
                    error!("Iteration {} failed: {}", iteration, e);
                    self.emit(LoopEvent::Error {
                        location: format!("iteration_{}", iteration),
                        message: e.to_string(),
                    });
                    if config.stop_on_failure {
                        break;
                    }
                    continue;
                }
            };

            // Save report
            self.save_report(&report).await?;
            reports.push(report.clone());

            self.emit(LoopEvent::IterationComplete {
                iteration,
                pass_rates: report.pass_rate_by_level_string(),
                promotions: report.lessons_promoted.iter().map(|(id, _, _)| id.clone()).collect(),
                demotions: report.lessons_demoted.iter().map(|(id, _, _)| id.clone()).collect(),
            });
        }

        // Update state
        {
            let mut state = self.state.write().await;
            *state = LoopState::Stopped;
        }

        self.emit(LoopEvent::RunStopped {
            run_id,
            reason: if *self.state.read().await == LoopState::Stopping {
                "User requested stop".to_string()
            } else {
                "Completed".to_string()
            },
        });

        // Reset to idle
        {
            let mut state = self.state.write().await;
            *state = LoopState::Idle;
        }

        Ok(reports)
    }

    async fn run_iteration(&self, config: &LoopConfig, iteration: u32) -> Result<IterationReport> {
        let started_at = Utc::now();
        let start_instant = std::time::Instant::now();

        // Run baseline
        let run_config = RunConfig {
            levels: config.levels.clone(),
            concurrency: config.concurrency,
            ..Default::default()
        };
        
        let results_before = self.runner.run_suite(&run_config).await?;
        let pass_rate_before = results_before.pass_rate();
        let failures_before = results_before.failures();

        info!(
            "Iteration {} baseline: {:.1}% pass rate ({} failures)",
            iteration,
            pass_rate_before * 100.0,
            failures_before.len()
        );

        // For each failure, attempt recovery and postmortem
        let mut lessons_staged = Vec::new();
        let mut tests_auto_minted = Vec::new();

        for failure in &failures_before {
            debug!("Processing failure: {}", failure.test_id);

            // TODO: Run recovery agent
            // TODO: Run postmortem to extract lessons
            // TODO: Auto-mint regression test

            // Placeholder for lesson staging
            if let Some(sig) = failure.failure_signature() {
                debug!("Failure signature: {}", sig);
            }
        }

        // Re-run suite with lessons available
        let results_after = self.runner.run_suite(&run_config).await?;
        let pass_rate_after = results_after.pass_rate();
        let failures_after = results_after.failures();

        info!(
            "Iteration {} after lessons: {:.1}% pass rate ({} failures)",
            iteration,
            pass_rate_after * 100.0,
            failures_after.len()
        );

        // Run promotion
        let mut lessons_promoted = Vec::new();
        let mut lessons_demoted = Vec::new();
        let mut lessons_rolled_back = Vec::new();

        // Rollback gate: if pass rate didn't improve, revert lessons from this iteration
        if pass_rate_after <= pass_rate_before && !lessons_staged.is_empty() {
            warn!(
                "Iteration {} did not improve pass rate, rolling back {} lessons",
                iteration,
                lessons_staged.len()
            );
            lessons_rolled_back = lessons_staged.clone();
            lessons_staged.clear();
        }

        let completed_at = Utc::now();
        let duration_ms = start_instant.elapsed().as_millis() as u64;

        Ok(IterationReport {
            iteration,
            started_at,
            completed_at,
            duration_ms,
            pass_rate_before,
            pass_rate_after,
            failures_before: failures_before.len(),
            failures_after: failures_after.len(),
            lessons_staged,
            lessons_promoted,
            lessons_demoted,
            lessons_rolled_back,
            tests_auto_minted,
        })
    }

    async fn save_report(&self, report: &IterationReport) -> Result<()> {
        let dir = self.foundry_path.join("runs").join(format!("iter-{}", report.iteration));
        fs::create_dir_all(&dir).await?;

        // Save JSON
        let json_path = dir.join("report.json");
        let json = serde_json::to_string_pretty(report)?;
        fs::write(&json_path, json).await?;

        // Save Markdown
        let md_path = dir.join("report.md");
        let md = self.format_report_markdown(report);
        fs::write(&md_path, md).await?;

        info!("Saved iteration {} report to {:?}", report.iteration, dir);
        Ok(())
    }

    fn format_report_markdown(&self, report: &IterationReport) -> String {
        let mut md = String::new();
        md.push_str(&format!("# Iteration {} Report\n\n", report.iteration));
        md.push_str(&format!("**Started:** {}\n", report.started_at));
        md.push_str(&format!("**Completed:** {}\n", report.completed_at));
        md.push_str(&format!("**Duration:** {}ms\n\n", report.duration_ms));

        md.push_str("## Pass Rates\n\n");
        md.push_str(&format!("- Before: {:.1}%\n", report.pass_rate_before * 100.0));
        md.push_str(&format!("- After: {:.1}%\n\n", report.pass_rate_after * 100.0));

        md.push_str("## Failures\n\n");
        md.push_str(&format!("- Before: {}\n", report.failures_before));
        md.push_str(&format!("- After: {}\n\n", report.failures_after));

        if !report.lessons_staged.is_empty() {
            md.push_str("## Lessons Staged\n\n");
            for lesson in &report.lessons_staged {
                md.push_str(&format!("- {}\n", lesson));
            }
            md.push_str("\n");
        }

        if !report.lessons_promoted.is_empty() {
            md.push_str("## Lessons Promoted\n\n");
            for (id, from, to) in &report.lessons_promoted {
                md.push_str(&format!("- {} ({} -> {})\n", id, from, to));
            }
            md.push_str("\n");
        }

        if !report.lessons_rolled_back.is_empty() {
            md.push_str("## Lessons Rolled Back\n\n");
            for lesson in &report.lessons_rolled_back {
                md.push_str(&format!("- {}\n", lesson));
            }
            md.push_str("\n");
        }

        md
    }

    fn emit(&self, event: LoopEvent) {
        let _ = self.event_tx.send(event);
    }
}

impl IterationReport {
    fn pass_rate_by_level_string(&self) -> std::collections::HashMap<String, f64> {
        // This is a simplified version - in practice we'd track by level
        let mut map = std::collections::HashMap::new();
        map.insert("overall".to_string(), self.pass_rate_after);
        map
    }
}
