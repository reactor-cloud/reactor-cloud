use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use studio_agent::AgentRunner;
use studio_protocol::{ConversationId, StreamChunk};
use studio_providers::{LlmProvider, OpenRouterProvider, ProviderConfig};
use studio_storage::{AgentLoader, ReactorPaths};

use crate::AppState;

const KEYRING_SERVICE: &str = "reactor-studio";
const OPENROUTER_KEY_NAME: &str = "openrouter_api_key";

/// Active agent session
struct AgentSession {
    cancellation_token: CancellationToken,
}

/// State for managing agent sessions (wrapped in Arc for cloning)
#[derive(Clone)]
pub struct AgentState {
    sessions: Arc<Mutex<HashMap<String, AgentSession>>>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Summary of an agent for the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub model: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent error: {0}")]
    Agent(String),
    #[error("OpenRouter API key not configured. Open Settings to add one.")]
    ProviderNotConfigured,
    #[error("Workspace not open")]
    WorkspaceNotOpen,
    #[error("Session not found")]
    SessionNotFound,
}

impl Serialize for AgentError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Try to get API key from multiple sources:
/// 1. OS keyring (reactor-studio/openrouter_api_key)
/// 2. Credentials file (.reactor/credentials.json)
/// 3. Environment variable (OPENROUTER_API_KEY)
fn get_api_key(workspace_path: &std::path::Path) -> Option<String> {
    // Try OS keyring first
    eprintln!("[get_api_key] Trying keyring...");
    if let Ok(entry) = Entry::new(KEYRING_SERVICE, OPENROUTER_KEY_NAME) {
        if let Ok(key) = entry.get_password() {
            eprintln!("[get_api_key] Found in keyring");
            return Some(key);
        } else {
            eprintln!("[get_api_key] Keyring entry exists but get_password failed");
        }
    } else {
        eprintln!("[get_api_key] Failed to create keyring entry");
    }
    
    // Try credentials file
    eprintln!("[get_api_key] Trying credentials file...");
    let creds_path = workspace_path.join(".reactor/credentials.json");
    if let Ok(content) = std::fs::read_to_string(&creds_path) {
        if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(key) = creds.get("openrouter").and_then(|v| v.as_str()) {
                eprintln!("[get_api_key] Found in credentials file");
                return Some(key.to_string());
            }
        }
    }
    
    // Try environment variable
    eprintln!("[get_api_key] Trying environment variable...");
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
        eprintln!("[get_api_key] Found in environment");
        return Some(key);
    }
    
    eprintln!("[get_api_key] No API key found in any source");
    None
}

#[tauri::command]
pub async fn agent_list(app_state: State<'_, AppState>) -> Result<Vec<AgentSummary>, AgentError> {
    let workspaces = app_state.workspaces.lock().unwrap();

    // Get the first workspace path (in Phase 1, we only have one window)
    let workspace_path = workspaces
        .keys()
        .next()
        .ok_or(AgentError::WorkspaceNotOpen)?
        .clone();
    drop(workspaces);

    let paths = ReactorPaths::new(&workspace_path);
    let loader = AgentLoader::new(paths);

    let agents = loader
        .list_all()
        .map_err(|e| AgentError::Agent(e.to_string()))?;

    Ok(agents
        .into_iter()
        .map(|a| AgentSummary {
            id: a.id,
            name: a.name,
            color: a.color,
            icon: a.icon,
            model: a.model,
        })
        .collect())
}

#[tauri::command]
pub async fn agent_send(
    app_handle: AppHandle,
    agent_state: State<'_, AgentState>,
    app_state: State<'_, AppState>,
    agent_id: String,
    conversation_id: String,
    message: String,
) -> Result<(), AgentError> {
    eprintln!("[agent_send] Called with agent_id={}, conversation_id={}", agent_id, conversation_id);
    
    // Get workspace path (scoped to ensure guard is dropped before async)
    let (workspace_path, workspace_name) = {
        let workspaces = app_state.workspaces.lock().unwrap();
        let path = workspaces
            .keys()
            .next()
            .ok_or(AgentError::WorkspaceNotOpen)?
            .clone();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_string();
        eprintln!("[agent_send] Workspace: {:?}", path);
        (path, name)
    };

    // Get API key - try multiple sources
    eprintln!("[agent_send] Getting API key");
    let api_key = get_api_key(&workspace_path).ok_or_else(|| {
        eprintln!("[agent_send] No API key found in any source");
        AgentError::ProviderNotConfigured
    })?;
    eprintln!("[agent_send] Got API key (length: {})", api_key.len());

    // Load agent config to get the model
    eprintln!("[agent_send] Loading agent config");
    let paths = ReactorPaths::new(&workspace_path);
    let loader = AgentLoader::new(paths);
    let agent_config = loader
        .load(&agent_id)
        .map_err(|e| {
            eprintln!("[agent_send] Failed to load agent: {}", e);
            AgentError::Agent(format!("Failed to load agent: {}", e))
        })?;
    eprintln!("[agent_send] Agent config loaded, model: {}", agent_config.model);

    // Create cancellation token
    let cancellation_token = CancellationToken::new();

    // Store session
    {
        let mut sessions = agent_state.sessions.lock().await;
        sessions.insert(
            conversation_id.clone(),
            AgentSession {
                cancellation_token: cancellation_token.clone(),
            },
        );
    }

    // Create provider with agent's configured model
    eprintln!("[agent_send] Creating provider with model: {}", agent_config.model);
    let provider: Arc<dyn LlmProvider> = Arc::new(OpenRouterProvider::new(ProviderConfig {
        api_key,
        model: agent_config.model.clone(),
        base_url: None,
    }));

    // Create runner
    let runner = AgentRunner::new(
        workspace_path.to_string_lossy().to_string(),
        workspace_name,
    );

    // Run agent
    eprintln!("[agent_send] Starting agent runner");
    let conv_id = ConversationId::from_string(&conversation_id);
    let mut rx = runner
        .run(&agent_id, &conv_id, &message, provider, cancellation_token)
        .await
        .map_err(|e| {
            eprintln!("[agent_send] Runner failed: {}", e);
            AgentError::Agent(e.to_string())
        })?;
    eprintln!("[agent_send] Runner started, spawning event emitter");

    // Clone AgentState for the spawned task
    let conv_id_clone = conversation_id.clone();
    let agent_state_clone = (*agent_state).clone();

    tokio::spawn(async move {
        eprintln!("[agent_send] Event emit task started");
        while let Some(chunk) = rx.recv().await {
            eprintln!("[agent_send] Received chunk: {:?}", std::mem::discriminant(&chunk));
            // Emit events with appropriate payloads for each type
            match &chunk {
                StreamChunk::Error { message } => {
                    eprintln!("[agent_send] Emitting error: {}", message);
                    // Error event expects { conversationId, error }
                    let payload = serde_json::json!({
                        "conversationId": conv_id_clone,
                        "error": message,
                    });
                    if let Err(e) = app_handle.emit("agent:error", payload) {
                        eprintln!("[agent_send] Failed to emit error: {:?}", e);
                    }
                }
                StreamChunk::Done { .. } => {
                    eprintln!("[agent_send] Emitting complete");
                    // Complete event expects { conversationId }
                    let payload = serde_json::json!({
                        "conversationId": conv_id_clone,
                    });
                    if let Err(e) = app_handle.emit("agent:complete", payload) {
                        eprintln!("[agent_send] Failed to emit complete: {:?}", e);
                    }
                }
                _ => {
                    // All other chunk types go via agent:chunk with { conversationId, chunk }
                    let payload = serde_json::json!({
                        "conversationId": conv_id_clone,
                        "chunk": chunk,
                    });
                    eprintln!("[agent_send] Emitting chunk: {}", serde_json::to_string(&payload).unwrap_or_default());
                    if let Err(e) = app_handle.emit("agent:chunk", payload) {
                        eprintln!("[agent_send] Failed to emit chunk: {:?}", e);
                    }
                }
            }

            // Clean up session on completion
            if matches!(chunk, StreamChunk::Done { .. } | StreamChunk::Error { .. }) {
                let mut sessions = agent_state_clone.sessions.lock().await;
                sessions.remove(&conv_id_clone);
            }
        }
        eprintln!("[agent_send] Event emit task ended");
    });

    Ok(())
}

#[tauri::command]
pub async fn agent_cancel(
    agent_state: State<'_, AgentState>,
    conversation_id: String,
) -> Result<(), AgentError> {
    let sessions = agent_state.sessions.lock().await;
    let session = sessions
        .get(&conversation_id)
        .ok_or(AgentError::SessionNotFound)?;

    session.cancellation_token.cancel();
    Ok(())
}

#[tauri::command]
pub async fn tool_approve(
    _conversation_id: String,
    _tool_call_id: String,
    _approved: bool,
) -> Result<(), AgentError> {
    // Tool approval will be implemented in a future phase
    // For now, tools auto-approve
    Ok(())
}
