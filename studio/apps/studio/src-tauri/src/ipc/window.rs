use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub workspace_path: Option<String>,
    pub selected_agent_id: Option<String>,
    pub active_conversation_id: Option<String>,
    pub active_task_id: Option<String>,
    pub file_browser_open: Option<bool>,
}

#[tauri::command]
pub async fn window_get_state() -> WindowState {
    // In Phase 0, we return defaults.
    // Phase 1+ will use tauri-plugin-store for persistence.
    WindowState::default()
}

#[tauri::command]
pub async fn window_set_state(state: WindowState) -> Result<(), String> {
    // In Phase 0, this is a no-op.
    // Phase 1+ will persist to tauri-plugin-store.
    Ok(())
}
