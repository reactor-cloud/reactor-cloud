use serde::{Deserialize, Serialize};
use tauri::State;

use studio_protocol::{AgentId, ConversationId, Message};
use studio_storage::{ConversationStore, ConversationSummary, ReactorPaths, StorageError};

use crate::AppState;

#[derive(Debug, thiserror::Error)]
pub enum ConversationError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Workspace not open")]
    WorkspaceNotOpen,
}

impl Serialize for ConversationError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn get_store(app_state: &AppState) -> Result<ConversationStore, ConversationError> {
    let workspaces = app_state.workspaces.lock().unwrap();
    let workspace_path = workspaces
        .keys()
        .next()
        .ok_or(ConversationError::WorkspaceNotOpen)?
        .clone();
    drop(workspaces);

    let paths = ReactorPaths::new(&workspace_path);
    Ok(ConversationStore::new(paths))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInfo {
    pub id: String,
    pub agent_id: String,
    pub title: String,
    pub created: String,
    pub updated: String,
    pub message_count: usize,
}

impl From<ConversationSummary> for ConversationInfo {
    fn from(s: ConversationSummary) -> Self {
        Self {
            id: s.id,
            agent_id: s.agent_id,
            title: s.title,
            created: s.created.to_rfc3339(),
            updated: s.updated.to_rfc3339(),
            message_count: s.message_count,
        }
    }
}

#[tauri::command]
pub async fn conversation_list(
    app_state: State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<ConversationInfo>, ConversationError> {
    let store = get_store(&app_state)?;
    let agent = AgentId::new(&agent_id);

    store
        .list(&agent)
        .map(|list| list.into_iter().map(ConversationInfo::from).collect())
        .map_err(|e: StorageError| ConversationError::Storage(e.to_string()))
}

#[tauri::command]
pub async fn conversation_list_all(
    app_state: State<'_, AppState>,
) -> Result<Vec<ConversationInfo>, ConversationError> {
    let store = get_store(&app_state)?;

    store
        .list_all()
        .map(|list| list.into_iter().map(ConversationInfo::from).collect())
        .map_err(|e: StorageError| ConversationError::Storage(e.to_string()))
}

#[tauri::command]
pub async fn conversation_create(
    app_state: State<'_, AppState>,
    agent_id: String,
    title: Option<String>,
) -> Result<String, ConversationError> {
    let store = get_store(&app_state)?;
    let agent = AgentId::new(&agent_id);

    store
        .create(&agent, title)
        .map(|id| id.as_str().to_string())
        .map_err(|e: StorageError| ConversationError::Storage(e.to_string()))
}

#[tauri::command]
pub async fn conversation_messages(
    app_state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<Message>, ConversationError> {
    let store = get_store(&app_state)?;
    let conv_id = ConversationId::from_string(&conversation_id);

    store
        .read_messages(&conv_id)
        .map_err(|e: StorageError| ConversationError::Storage(e.to_string()))
}

#[tauri::command]
pub async fn conversation_delete(
    app_state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), ConversationError> {
    let store = get_store(&app_state)?;
    let conv_id = ConversationId::from_string(&conversation_id);

    store
        .delete(&conv_id)
        .map_err(|e: StorageError| ConversationError::Storage(e.to_string()))
}
