use serde::Serialize;
use studio_tracing::{ConversationTrace, TraceSummary, TraceStore};
use tauri::State;

#[derive(Clone, Default)]
pub struct TraceState {
    pub store: TraceStore,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceGetResult {
    pub success: bool,
    pub trace: Option<ConversationTrace>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceListResult {
    pub success: bool,
    pub summaries: Vec<TraceSummary>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn trace_get(
    conversation_id: String,
    state: State<'_, TraceState>,
) -> Result<TraceGetResult, String> {
    match state.store.get(&conversation_id).await {
        Some(trace) => Ok(TraceGetResult {
            success: true,
            trace: Some(trace),
            error: None,
        }),
        None => Ok(TraceGetResult {
            success: false,
            trace: None,
            error: Some(format!("No trace found for conversation: {}", conversation_id)),
        }),
    }
}

#[tauri::command]
pub async fn trace_list_conversations(
    state: State<'_, TraceState>,
) -> Result<TraceListResult, String> {
    let summaries = state.store.list().await;
    Ok(TraceListResult {
        success: true,
        summaries,
        error: None,
    })
}
