use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub path: PathBuf,
    pub name: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInfo {
    pub id: String,
    pub agent_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateSnapshot {
    pub workspace: Option<WorkspaceInfo>,
    pub selected_agent_id: Option<String>,
    pub active_conversation_id: Option<String>,
    pub agents: Vec<AgentInfo>,
    pub conversations: Vec<ConversationInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct AppStateInner {
    pub workspace: Option<WorkspaceInfo>,
    pub selected_agent_id: Option<String>,
    pub active_conversation_id: Option<String>,
    pub agents: Vec<AgentInfo>,
    pub conversations: HashMap<String, Vec<ConversationInfo>>,
    pub auth_token: String,
    pub port: u16,
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<AppStateInner>>,
    #[cfg(feature = "tauri-integration")]
    pub app_handle: Option<tauri::AppHandle>,
}

impl AppState {
    pub fn new(auth_token: String) -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppStateInner {
                auth_token,
                ..Default::default()
            })),
            #[cfg(feature = "tauri-integration")]
            app_handle: None,
        }
    }

    #[cfg(feature = "tauri-integration")]
    pub fn with_app_handle(mut self, handle: tauri::AppHandle) -> Self {
        self.app_handle = Some(handle);
        self
    }

    pub async fn get_snapshot(&self) -> AppStateSnapshot {
        let inner = self.inner.read().await;
        let conversations: Vec<ConversationInfo> = inner
            .conversations
            .values()
            .flatten()
            .cloned()
            .collect();

        AppStateSnapshot {
            workspace: inner.workspace.clone(),
            selected_agent_id: inner.selected_agent_id.clone(),
            active_conversation_id: inner.active_conversation_id.clone(),
            agents: inner.agents.clone(),
            conversations,
        }
    }

    pub async fn get_auth_token(&self) -> String {
        self.inner.read().await.auth_token.clone()
    }

    pub async fn set_port(&self, port: u16) {
        self.inner.write().await.port = port;
    }

    pub async fn get_port(&self) -> u16 {
        self.inner.read().await.port
    }

    pub async fn set_workspace(&self, workspace: Option<WorkspaceInfo>) {
        self.inner.write().await.workspace = workspace;
    }

    pub async fn get_workspace(&self) -> Option<WorkspaceInfo> {
        self.inner.read().await.workspace.clone()
    }

    pub async fn set_selected_agent(&self, agent_id: Option<String>) {
        self.inner.write().await.selected_agent_id = agent_id;
    }

    pub async fn get_selected_agent(&self) -> Option<String> {
        self.inner.read().await.selected_agent_id.clone()
    }

    pub async fn set_active_conversation(&self, conv_id: Option<String>) {
        self.inner.write().await.active_conversation_id = conv_id;
    }

    pub async fn get_active_conversation(&self) -> Option<String> {
        self.inner.read().await.active_conversation_id.clone()
    }

    pub async fn set_agents(&self, agents: Vec<AgentInfo>) {
        self.inner.write().await.agents = agents;
    }

    pub async fn get_agents(&self) -> Vec<AgentInfo> {
        self.inner.read().await.agents.clone()
    }

    pub async fn add_conversation(&self, agent_id: &str, conversation: ConversationInfo) {
        let mut inner = self.inner.write().await;
        inner
            .conversations
            .entry(agent_id.to_string())
            .or_default()
            .push(conversation);
    }

    pub async fn get_conversations(&self, agent_id: &str) -> Vec<ConversationInfo> {
        self.inner
            .read()
            .await
            .conversations
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
    }
}
