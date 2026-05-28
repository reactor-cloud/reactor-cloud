use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use studio_protocol::Message;
use studio_task::{Phase, TaskId, TaskStore, TaskSummary, Task};

use crate::AppState;

#[derive(Debug, thiserror::Error)]
pub enum TaskIpcError {
    #[error("Task error: {0}")]
    Task(String),
    #[error("Workspace not open")]
    WorkspaceNotOpen,
}

impl Serialize for TaskIpcError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn get_store(app_state: &AppState) -> Result<TaskStore, TaskIpcError> {
    let workspaces = app_state.workspaces.lock().unwrap();
    let workspace_path = workspaces
        .keys()
        .next()
        .ok_or(TaskIpcError::WorkspaceNotOpen)?
        .clone();
    drop(workspaces);

    let reactor_path = workspace_path.join(".reactor");
    Ok(TaskStore::new(reactor_path))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    pub description: String,
    pub state: String,
    pub current_phase: String,
    pub phases: Vec<PhaseInfo>,
    pub progress: f32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseInfo {
    pub phase: String,
    pub status: String,
    pub conversation_id: Option<String>,
}

impl From<Task> for TaskInfo {
    fn from(task: Task) -> Self {
        let completed = task
            .phases
            .iter()
            .filter(|p| p.status == studio_task::PhaseStatus::Completed)
            .count();
        let total = task.phases.len();
        let progress = (completed as f32 / total as f32) * 100.0;

        Self {
            id: task.id.as_str().to_string(),
            title: task.title,
            description: task.description,
            state: format!("{:?}", task.state).to_lowercase(),
            current_phase: task.current_phase.name().to_string(),
            phases: task
                .phases
                .iter()
                .map(|p| PhaseInfo {
                    phase: p.phase.name().to_string(),
                    status: format!("{:?}", p.status).to_lowercase(),
                    conversation_id: p.conversation_id.clone(),
                })
                .collect(),
            progress,
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
        }
    }
}

#[tauri::command]
pub async fn task_list(
    app_state: State<'_, AppState>,
) -> Result<Vec<TaskSummary>, TaskIpcError> {
    let store = get_store(&app_state)?;
    store.list().map_err(|e| TaskIpcError::Task(e.to_string()))
}

#[tauri::command]
pub async fn task_create(
    app_state: State<'_, AppState>,
    title: String,
    description: Option<String>,
) -> Result<TaskInfo, TaskIpcError> {
    let store = get_store(&app_state)?;
    store
        .create(&title, description.as_deref())
        .map(TaskInfo::from)
        .map_err(|e| TaskIpcError::Task(e.to_string()))
}

#[tauri::command]
pub async fn task_get(
    app_state: State<'_, AppState>,
    task_id: String,
) -> Result<TaskInfo, TaskIpcError> {
    let store = get_store(&app_state)?;
    let id = TaskId::from_string(&task_id);
    store
        .get(&id)
        .map(TaskInfo::from)
        .map_err(|e| TaskIpcError::Task(e.to_string()))
}

#[tauri::command]
pub async fn task_advance(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
    task_id: String,
) -> Result<TaskInfo, TaskIpcError> {
    let store = get_store(&app_state)?;
    let id = TaskId::from_string(&task_id);
    let task = store
        .advance(&id)
        .map_err(|e| TaskIpcError::Task(e.to_string()))?;

    // Emit phase changed event
    let _ = app_handle.emit(
        "task:phase-changed",
        serde_json::json!({
            "taskId": task.id.as_str(),
            "phase": task.current_phase.name(),
        }),
    );

    Ok(TaskInfo::from(task))
}

#[tauri::command]
pub async fn task_phase_messages(
    app_state: State<'_, AppState>,
    task_id: String,
    phase: String,
) -> Result<Vec<Message>, TaskIpcError> {
    let store = get_store(&app_state)?;
    let id = TaskId::from_string(&task_id);

    let phase = match phase.to_lowercase().as_str() {
        "alignment" => Phase::Alignment,
        "planning" => Phase::Planning,
        "development" => Phase::Development,
        "testing" => Phase::Testing,
        "uat" => Phase::Uat,
        "deployment" => Phase::Deployment,
        _ => return Err(TaskIpcError::Task(format!("Unknown phase: {}", phase))),
    };

    store
        .phase_messages(&id, phase)
        .map_err(|e| TaskIpcError::Task(e.to_string()))
}

#[tauri::command]
pub async fn task_delete(
    app_state: State<'_, AppState>,
    task_id: String,
) -> Result<(), TaskIpcError> {
    let store = get_store(&app_state)?;
    let id = TaskId::from_string(&task_id);
    store
        .delete(&id)
        .map_err(|e| TaskIpcError::Task(e.to_string()))
}
