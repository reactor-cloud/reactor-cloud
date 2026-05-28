use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use studio_eval::{
    AutoIterationLoop, LoopConfig, LoopEvent, RunConfig, Runner, SuiteResult, TestLevel,
};
use studio_lessons::{Lesson, LessonId, LessonStore, LedgerWriter, Retriever, Tier};
// Note: studio_promotion types available when needed for tier transitions

/// Foundry state for managing the evaluation loop
#[derive(Clone)]
pub struct FoundryState {
    loop_instance: Arc<Mutex<Option<AutoIterationLoop>>>,
    foundry_path: PathBuf,
}

impl Default for FoundryState {
    fn default() -> Self {
        Self {
            loop_instance: Arc::new(Mutex::new(None)),
            foundry_path: PathBuf::from(".foundry"),
        }
    }
}

impl FoundryState {
    pub fn new(foundry_path: PathBuf) -> Self {
        Self {
            loop_instance: Arc::new(Mutex::new(None)),
            foundry_path,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FoundryError {
    #[error("Eval error: {0}")]
    Eval(#[from] studio_eval::EvalError),
    #[error("Lesson error: {0}")]
    Lesson(#[from] studio_lessons::LessonError),
    #[error("Promotion error: {0}")]
    Promotion(#[from] studio_promotion::PromotionError),
    #[error("Loop already running")]
    LoopAlreadyRunning,
    #[error("Loop not running")]
    LoopNotRunning,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for FoundryError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// Request/Response types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaselineRequest {
    pub levels: Vec<String>,
    pub concurrency: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaselineResponse {
    pub pass_rate: f64,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rates_by_level: HashMap<String, f64>,
    pub duration_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub levels: Vec<String>,
    pub max_iterations: Option<u32>,
    pub concurrency: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatusResponse {
    pub running: bool,
    pub iteration: Option<u32>,
    pub pass_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayRecordRequest {
    pub test_id: String,
    pub seed: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayPlayRequest {
    pub test_id: String,
    pub seed: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonsListRequest {
    pub tier: Option<String>,
    pub scope: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonSummary {
    pub id: String,
    pub title: String,
    pub tier: String,
    pub scope: String,
    pub kind: String,
    pub citations: u64,
    pub success_rate: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonDetail {
    pub lesson: Lesson,
    pub ledger_entries: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonsStatsResponse {
    pub total: usize,
    pub by_tier: HashMap<String, usize>,
    pub total_citations: u64,
    pub avg_success_rate: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub iteration: u32,
    pub pass_rate_before: f64,
    pub pass_rate_after: f64,
    pub completed_at: String,
}

fn parse_levels(levels: &[String]) -> Vec<TestLevel> {
    levels
        .iter()
        .filter_map(|s| TestLevel::from_str(s))
        .collect()
}

// Tauri commands

#[tauri::command]
pub async fn foundry_baseline(
    state: State<'_, FoundryState>,
    request: BaselineRequest,
) -> Result<BaselineResponse, FoundryError> {
    let runner = Runner::new(&state.foundry_path);
    let config = RunConfig {
        levels: parse_levels(&request.levels),
        concurrency: request.concurrency.unwrap_or(4),
        ..Default::default()
    };

    let result = runner.run_suite(&config).await?;

    let pass_rates_by_level: HashMap<String, f64> = result
        .pass_rate_by_level()
        .into_iter()
        .map(|(level, rate)| (level.as_str().to_string(), rate))
        .collect();

    Ok(BaselineResponse {
        pass_rate: result.pass_rate(),
        total: result.total(),
        passed: result.passed(),
        failed: result.failed(),
        pass_rates_by_level,
        duration_ms: result.duration_ms,
    })
}

#[tauri::command]
pub async fn foundry_run(
    app: AppHandle,
    state: State<'_, FoundryState>,
    request: RunRequest,
) -> Result<String, FoundryError> {
    let mut loop_guard = state.loop_instance.lock().await;

    if loop_guard.is_some() {
        return Err(FoundryError::LoopAlreadyRunning);
    }

    let loop_instance = AutoIterationLoop::new(&state.foundry_path);
    let run_id = uuid::Uuid::new_v4().to_string();

    // Subscribe to events and forward to frontend
    let mut receiver = loop_instance.subscribe();
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            let event_name = match &event {
                LoopEvent::RunStarted { .. } => "foundry:run_started",
                LoopEvent::TestResult { .. } => "foundry:test_result",
                LoopEvent::IterationComplete { .. } => "foundry:iteration_complete",
                LoopEvent::LessonStaged { .. } => "foundry:lesson_staged",
                LoopEvent::LessonPromoted { .. } => "foundry:lesson_promoted",
                LoopEvent::RunStopped { .. } => "foundry:run_stopped",
                LoopEvent::Error { .. } => "foundry:error",
            };
            let _ = app_clone.emit(event_name, &event);
        }
    });

    let config = LoopConfig {
        levels: parse_levels(&request.levels),
        max_iterations: request.max_iterations,
        concurrency: request.concurrency.unwrap_or(4),
        ..Default::default()
    };

    // Store the loop instance
    *loop_guard = Some(loop_instance);

    // Clone for the spawned task
    let loop_instance_for_run = state.loop_instance.clone();
    let foundry_path = state.foundry_path.clone();

    // Spawn the loop in a separate task
    tokio::spawn(async move {
        let loop_guard = loop_instance_for_run.lock().await;
        if let Some(ref loop_instance) = *loop_guard {
            let _ = loop_instance.run(config).await;
        }
        drop(loop_guard);

        // Clear the instance when done
        let mut guard = loop_instance_for_run.lock().await;
        *guard = None;
    });

    Ok(run_id)
}

#[tauri::command]
pub async fn foundry_stop(state: State<'_, FoundryState>) -> Result<(), FoundryError> {
    let loop_guard = state.loop_instance.lock().await;

    if let Some(ref loop_instance) = *loop_guard {
        loop_instance.request_stop().await;
        Ok(())
    } else {
        Err(FoundryError::LoopNotRunning)
    }
}

#[tauri::command]
pub async fn foundry_status(state: State<'_, FoundryState>) -> Result<RunStatusResponse, FoundryError> {
    let loop_guard = state.loop_instance.lock().await;

    if let Some(ref loop_instance) = *loop_guard {
        let loop_state = loop_instance.state().await;
        Ok(RunStatusResponse {
            running: loop_state == studio_eval::loop_driver::LoopState::Running,
            iteration: None, // Would need to track this
            pass_rate: None,
        })
    } else {
        Ok(RunStatusResponse {
            running: false,
            iteration: None,
            pass_rate: None,
        })
    }
}

#[tauri::command]
pub async fn foundry_replay_record(
    state: State<'_, FoundryState>,
    request: ReplayRecordRequest,
) -> Result<String, FoundryError> {
    // TODO: Implement replay recording
    let seed = request.seed.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    Ok(seed)
}

#[tauri::command]
pub async fn foundry_replay_play(
    state: State<'_, FoundryState>,
    request: ReplayPlayRequest,
) -> Result<(), FoundryError> {
    // TODO: Implement replay playback
    Ok(())
}

#[tauri::command]
pub async fn foundry_lessons_list(
    state: State<'_, FoundryState>,
    request: LessonsListRequest,
) -> Result<Vec<LessonSummary>, FoundryError> {
    let store = LessonStore::new(state.foundry_path.join("lessons"));
    let lessons = store.list_all().await?;

    let summaries: Vec<LessonSummary> = lessons
        .into_iter()
        .filter(|l| {
            // Filter by tier if specified
            if let Some(ref tier_str) = request.tier {
                let tier_matches = match tier_str.as_str() {
                    "T0" => l.tier == Tier::T0,
                    "T1" => l.tier == Tier::T1,
                    "T2" => l.tier == Tier::T2,
                    "T3" => l.tier == Tier::T3,
                    "T4" => l.tier == Tier::T4,
                    _ => true,
                };
                if !tier_matches {
                    return false;
                }
            }
            // Filter by domain if specified
            if let Some(ref domain) = request.domain {
                if !l.tags.iter().any(|t| t.contains(domain)) {
                    return false;
                }
            }
            true
        })
        .map(|l| {
            let kind = match &l.kind {
                studio_lessons::LessonKind::PromptDelta { .. } => "prompt_delta",
                studio_lessons::LessonKind::Heuristic { .. } => "heuristic",
                studio_lessons::LessonKind::SkillBundle { .. } => "skill_bundle",
                studio_lessons::LessonKind::ToolProposal { .. } => "tool_proposal",
                studio_lessons::LessonKind::AntiPattern { .. } => "anti_pattern",
            };
            let success_rate = l.success_rate();
            LessonSummary {
                id: l.id.0.clone(),
                title: l.title.clone(),
                tier: format!("{:?}", l.tier),
                scope: format!("{:?}", l.scope),
                kind: kind.to_string(),
                citations: l.citations,
                success_rate,
            }
        })
        .collect();

    Ok(summaries)
}

#[tauri::command]
pub async fn foundry_lessons_show(
    state: State<'_, FoundryState>,
    lesson_id: String,
) -> Result<LessonDetail, FoundryError> {
    let store = LessonStore::new(state.foundry_path.join("lessons"));
    let ledger = LedgerWriter::new(state.foundry_path.join("ledger.jsonl"));

    let id = LessonId::from_string(lesson_id);
    let lesson = store.load(&id).await?;

    let entries = ledger.entries_for_lesson(&id)?;
    let ledger_entries: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| serde_json::to_value(e).unwrap_or_default())
        .collect();

    Ok(LessonDetail {
        lesson,
        ledger_entries,
    })
}

#[tauri::command]
pub async fn foundry_lessons_stats(
    state: State<'_, FoundryState>,
) -> Result<LessonsStatsResponse, FoundryError> {
    let store = LessonStore::new(state.foundry_path.join("lessons"));
    let counts = store.count_by_tier().await?;
    let lessons = store.list_all().await?;

    let total = lessons.len();
    let by_tier: HashMap<String, usize> = counts
        .into_iter()
        .map(|(tier, count)| (format!("{:?}", tier), count))
        .collect();

    let total_citations: u64 = lessons.iter().map(|l| l.citations).sum();
    let avg_success_rate = if total > 0 {
        lessons.iter().map(|l| l.success_rate()).sum::<f64>() / total as f64
    } else {
        0.0
    };

    Ok(LessonsStatsResponse {
        total,
        by_tier,
        total_citations,
        avg_success_rate,
    })
}

#[tauri::command]
pub async fn foundry_reports_list(
    state: State<'_, FoundryState>,
) -> Result<Vec<ReportSummary>, FoundryError> {
    let runs_dir = state.foundry_path.join("runs");
    if !runs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut reports = Vec::new();
    let mut entries = tokio::fs::read_dir(&runs_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str());
            if let Some(name) = name {
                if name.starts_with("iter-") {
                    let report_path = path.join("report.json");
                    if report_path.exists() {
                        if let Ok(content) = tokio::fs::read_to_string(&report_path).await {
                            if let Ok(report) = serde_json::from_str::<studio_eval::IterationReport>(&content) {
                                reports.push(ReportSummary {
                                    iteration: report.iteration,
                                    pass_rate_before: report.pass_rate_before,
                                    pass_rate_after: report.pass_rate_after,
                                    completed_at: report.completed_at.to_rfc3339(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by iteration
    reports.sort_by(|a, b| b.iteration.cmp(&a.iteration));

    Ok(reports)
}
