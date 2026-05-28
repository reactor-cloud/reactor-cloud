use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsQuery {
    pub cat: Option<String>,
    pub level: Option<String>,
    pub limit: Option<usize>,
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub category: String,
    pub event: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsResponse {
    pub entries: Vec<LogEntry>,
    pub total: usize,
    pub truncated: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/logs", get(get_logs))
}

async fn get_logs(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    let workspace = match state.get_workspace().await {
        Some(ws) => ws,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(LogsResponse {
                    entries: vec![],
                    total: 0,
                    truncated: false,
                }),
            );
        }
    };

    let category = query.cat.unwrap_or_else(|| "agent".to_string());
    let limit = query.limit.unwrap_or(100);

    let log_file = match category.as_str() {
        "agent" => workspace.path.join(".reactor/logs/agent.jsonl"),
        "app" => workspace.path.join(".reactor/logs/app.jsonl"),
        _ => workspace.path.join(".reactor/logs/agent.jsonl"),
    };

    let entries = read_log_file(&log_file, limit, query.level.as_deref(), query.since.as_deref())
        .await
        .unwrap_or_default();

    let total = entries.len();

    (
        StatusCode::OK,
        Json(LogsResponse {
            entries,
            total,
            truncated: false,
        }),
    )
}

async fn read_log_file(
    path: &PathBuf,
    limit: usize,
    level_filter: Option<&str>,
    since_filter: Option<&str>,
) -> Option<Vec<LogEntry>> {
    let content = tokio::fs::read_to_string(path).await.ok()?;

    let entries: Vec<LogEntry> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .filter(|entry: &LogEntry| {
            if let Some(level) = level_filter {
                if entry.level.to_lowercase() != level.to_lowercase() {
                    return false;
                }
            }
            if let Some(since) = since_filter {
                if entry.timestamp < since.to_string() {
                    return false;
                }
            }
            true
        })
        .take(limit)
        .collect();

    Some(entries)
}
