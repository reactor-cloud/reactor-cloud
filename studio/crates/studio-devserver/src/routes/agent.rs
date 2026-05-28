use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{state::ConversationInfo, AppState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectAgentRequest {
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewConversationRequest {
    pub agent_id: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewConversationResponse {
    pub success: bool,
    pub conversation_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub conversation_id: String,
    pub message: String,
    /// If true, run the agent and wait for response (blocking)
    pub run_agent: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResponse {
    pub success: bool,
    pub message_id: Option<String>,
    /// If agent was run, this contains the assistant's response
    pub response: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitResponseResult {
    pub success: bool,
    pub final_text: Option<String>,
    pub tool_sequence: Vec<ToolCallSummary>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallSummary {
    pub name: String,
    pub args: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub status: String,
    pub duration_ms: Option<u64>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/agents", get(list_agents))
        .route("/select-agent", post(select_agent))
        .route("/new-conversation", post(new_conversation))
        .route("/send-message", post(send_message))
        .route("/wait-response", get(wait_response))
        .route("/conversations", get(list_conversations))
        .route("/messages", get(get_messages))
}

async fn list_agents(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.get_agents().await;
    Json(agents)
}

async fn select_agent(
    State(state): State<AppState>,
    Json(request): Json<SelectAgentRequest>,
) -> impl IntoResponse {
    state.set_selected_agent(Some(request.agent_id.clone())).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "agentId": request.agent_id
        })),
    )
}

async fn new_conversation(
    State(state): State<AppState>,
    Json(request): Json<NewConversationRequest>,
) -> impl IntoResponse {
    // Get workspace
    let workspace = match state.get_workspace().await {
        Some(ws) => ws,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(NewConversationResponse {
                    success: false,
                    conversation_id: None,
                    error: Some("No workspace open. Call /open-workspace first.".to_string()),
                }),
            );
        }
    };

    let agent_id = match request.agent_id.or(state.get_selected_agent().await) {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(NewConversationResponse {
                    success: false,
                    conversation_id: None,
                    error: Some("No agent selected. Call /select-agent first.".to_string()),
                }),
            );
        }
    };

    // Create conversation using studio-storage
    let paths = studio_storage::ReactorPaths::new(&workspace.path);
    let store = studio_storage::ConversationStore::new(paths);
    let agent = studio_protocol::AgentId::new(&agent_id);

    let conversation_id = match store.create(&agent, request.title) {
        Ok(id) => id.as_str().to_string(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(NewConversationResponse {
                    success: false,
                    conversation_id: None,
                    error: Some(format!("Failed to create conversation: {}", e)),
                }),
            );
        }
    };

    let now = chrono::Utc::now().to_rfc3339();
    let conversation = ConversationInfo {
        id: conversation_id.clone(),
        agent_id: agent_id.clone(),
        title: None,
        created_at: now.clone(),
        updated_at: now,
        message_count: 0,
    };

    state.add_conversation(&agent_id, conversation).await;
    state.set_active_conversation(Some(conversation_id.clone())).await;

    (
        StatusCode::OK,
        Json(NewConversationResponse {
            success: true,
            conversation_id: Some(conversation_id),
            error: None,
        }),
    )
}

async fn send_message(
    State(state): State<AppState>,
    Json(request): Json<SendMessageRequest>,
) -> impl IntoResponse {
    // Get workspace
    let workspace = match state.get_workspace().await {
        Some(ws) => ws,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(SendMessageResponse {
                    success: false,
                    message_id: None,
                    response: None,
                    error: Some("No workspace open".to_string()),
                }),
            );
        }
    };

    // Get selected agent
    let agent_id = match state.get_selected_agent().await {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(SendMessageResponse {
                    success: false,
                    message_id: None,
                    response: None,
                    error: Some("No agent selected".to_string()),
                }),
            );
        }
    };

    let paths = studio_storage::ReactorPaths::new(&workspace.path);
    let store = studio_storage::ConversationStore::new(paths.clone());
    let conv_id = studio_protocol::ConversationId::from_string(&request.conversation_id);

    // If NOT running agent, save message directly.
    // If running agent, the runner will save the message.
    let message_id = if !request.run_agent.unwrap_or(false) {
        let user_message = studio_protocol::Message::user(&request.message);
        let id = user_message.id.0.clone();
        if let Err(e) = store.append_message(&conv_id, &user_message) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SendMessageResponse {
                    success: false,
                    message_id: None,
                    response: None,
                    error: Some(format!("Failed to save message: {}", e)),
                }),
            );
        }
        Some(id)
    } else {
        None
    };

    // If run_agent is true, run the agent and collect response
    let response_text = if request.run_agent.unwrap_or(false) {
        match run_agent_for_message(&workspace, &agent_id, &request.conversation_id, &request.message).await {
            Ok(response) => Some(response),
            Err(e) => {
                return (
                    StatusCode::OK,
                    Json(SendMessageResponse {
                        success: false,
                        message_id: None,
                        response: None,
                        error: Some(format!("Agent failed: {}", e)),
                    }),
                );
            }
        }
    } else {
        None
    };

    (
        StatusCode::OK,
        Json(SendMessageResponse {
            success: true,
            message_id,
            response: response_text,
            error: None,
        }),
    )
}

async fn run_agent_for_message(
    workspace: &crate::WorkspaceInfo,
    agent_id: &str,
    conversation_id: &str,
    user_message: &str,
) -> Result<String, String> {
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    // Get API key
    let paths = studio_storage::ReactorPaths::new(&workspace.path);
    let api_key = get_api_key(&paths).await
        .ok_or_else(|| "No API key found. Set OPENROUTER_API_KEY env var.".to_string())?;

    // Load agent config to get model
    let loader = studio_storage::AgentLoader::new(paths.clone());
    let agent_config = loader.load(agent_id)
        .map_err(|e| format!("Failed to load agent: {}", e))?;

    // Create provider
    let provider_config = studio_providers::ProviderConfig {
        api_key,
        model: agent_config.model.clone(),
        base_url: None,
    };
    let provider: Arc<dyn studio_providers::LlmProvider> = 
        Arc::new(studio_providers::OpenRouterProvider::new(provider_config));

    // Create runner
    let runner = studio_agent::AgentRunner::new(
        workspace.path.to_string_lossy(),
        &workspace.name,
    );

    let conv_id = studio_protocol::ConversationId::from_string(conversation_id);
    let cancellation = CancellationToken::new();

    // Run agent and collect response
    let mut rx = runner
        .run(agent_id, &conv_id, user_message, provider, cancellation)
        .await
        .map_err(|e| format!("Agent run failed: {}", e))?;

    let mut response_text = String::new();
    while let Some(chunk) = rx.recv().await {
        if let studio_protocol::StreamChunk::Text { content } = chunk {
            response_text.push_str(&content);
        }
    }

    Ok(response_text)
}

async fn get_api_key(paths: &studio_storage::ReactorPaths) -> Option<String> {
    // Try OS keyring first (same source as Tauri app Settings)
    if let Ok(entry) = keyring::Entry::new("reactor-studio", "openrouter_api_key") {
        if let Ok(key) = entry.get_password() {
            return Some(key);
        }
    }

    // Try credentials store (legacy/workspace-specific)
    let creds_path = paths.project_root().join(".reactor/credentials.json");
    if let Ok(content) = tokio::fs::read_to_string(creds_path).await {
        if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(key) = creds.get("openrouter").and_then(|v| v.as_str()) {
                return Some(key.to_string());
            }
        }
    }
    
    // Fallback to environment variable
    std::env::var("OPENROUTER_API_KEY").ok()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsQuery {
    pub agent_id: Option<String>,
}

async fn list_conversations(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<ListConversationsQuery>,
) -> impl IntoResponse {
    let workspace = match state.get_workspace().await {
        Some(ws) => ws,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No workspace open"
                })),
            );
        }
    };

    let paths = studio_storage::ReactorPaths::new(&workspace.path);
    let store = studio_storage::ConversationStore::new(paths);

    let conversations = if let Some(agent_id) = query.agent_id {
        let agent = studio_protocol::AgentId::new(&agent_id);
        store.list(&agent).unwrap_or_default()
    } else {
        store.list_all().unwrap_or_default()
    };

    let infos: Vec<serde_json::Value> = conversations
        .into_iter()
        .map(|c| serde_json::json!({
            "id": c.id,
            "agentId": c.agent_id,
            "title": c.title,
            "created": c.created.to_rfc3339(),
            "updated": c.updated.to_rfc3339(),
            "messageCount": c.message_count,
        }))
        .collect();

    (StatusCode::OK, Json(serde_json::json!(infos)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMessagesQuery {
    pub conversation_id: String,
}

async fn get_messages(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<GetMessagesQuery>,
) -> impl IntoResponse {
    let workspace = match state.get_workspace().await {
        Some(ws) => ws,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No workspace open"
                })),
            );
        }
    };

    let paths = studio_storage::ReactorPaths::new(&workspace.path);
    let store = studio_storage::ConversationStore::new(paths);
    let conv_id = studio_protocol::ConversationId::from_string(&query.conversation_id);

    match store.read_messages(&conv_id) {
        Ok(messages) => (StatusCode::OK, Json(serde_json::json!(messages))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to read messages: {}", e)
            })),
        ),
    }
}

async fn wait_response(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Implement SSE streaming for response waiting
    Json(WaitResponseResult {
        success: true,
        final_text: Some("Response placeholder".to_string()),
        tool_sequence: vec![],
        duration_ms: 0,
        error: None,
    })
}
