use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

use crate::{AppState, WorkspaceInfo};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWorkspaceRequest {
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWorkspaceResponse {
    pub success: bool,
    pub workspace: Option<WorkspaceInfo>,
    pub error: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/open-workspace", post(open_workspace))
}

fn create_default_config(project_name: &str) -> String {
    format!(
        r#"[project]
name = "{}"
created = "{}"

[agents]
default = "coder"

[providers]
default = "openrouter"

[tasks]
phases = [
  "alignment",
  "planning",
  "development",
  "testing",
  "uat",
  "deployment",
]

[index]
enabled = true
"#,
        project_name,
        chrono::Utc::now().to_rfc3339()
    )
}

fn scaffold_reactor_dir(path: &PathBuf) -> Result<(), String> {
    let reactor_dir = path.join(".reactor");

    if !reactor_dir.exists() {
        fs::create_dir_all(&reactor_dir)
            .map_err(|e| format!("Failed to create .reactor directory: {}", e))?;

        // Create subdirectories
        fs::create_dir_all(reactor_dir.join("agents"))
            .map_err(|e| format!("Failed to create agents directory: {}", e))?;
        fs::create_dir_all(reactor_dir.join("conversations"))
            .map_err(|e| format!("Failed to create conversations directory: {}", e))?;
        fs::create_dir_all(reactor_dir.join("tasks"))
            .map_err(|e| format!("Failed to create tasks directory: {}", e))?;
        fs::create_dir_all(reactor_dir.join("memory"))
            .map_err(|e| format!("Failed to create memory directory: {}", e))?;
        fs::create_dir_all(reactor_dir.join("cache"))
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        fs::create_dir_all(reactor_dir.join("logs"))
            .map_err(|e| format!("Failed to create logs directory: {}", e))?;

        // Create config.toml
        let project_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");

        let config_path = reactor_dir.join("config.toml");
        fs::write(&config_path, create_default_config(project_name))
            .map_err(|e| format!("Failed to write config: {}", e))?;
    }

    Ok(())
}

async fn open_workspace(
    State(state): State<AppState>,
    Json(request): Json<OpenWorkspaceRequest>,
) -> impl IntoResponse {
    let path = PathBuf::from(&request.path);

    if !path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(OpenWorkspaceResponse {
                success: false,
                workspace: None,
                error: Some(format!("Path does not exist: {}", request.path)),
            }),
        );
    }

    if !path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(OpenWorkspaceResponse {
                success: false,
                workspace: None,
                error: Some(format!("Path is not a directory: {}", request.path)),
            }),
        );
    }

    // Canonicalize path
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(OpenWorkspaceResponse {
                    success: false,
                    workspace: None,
                    error: Some(format!("Failed to canonicalize path: {}", e)),
                }),
            );
        }
    };

    // Scaffold .reactor/ directory
    if let Err(e) = scaffold_reactor_dir(&canonical) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(OpenWorkspaceResponse {
                success: false,
                workspace: None,
                error: Some(e),
            }),
        );
    }

    // Seed default agents using studio-storage
    let paths = studio_storage::ReactorPaths::new(&canonical);
    let agent_loader = studio_storage::AgentLoader::new(paths);
    if let Err(e) = agent_loader.seed_inline_defaults() {
        tracing::warn!("Failed to seed default agents: {}", e);
    }

    // Migrate old model IDs to valid OpenRouter slugs
    match agent_loader.migrate_models() {
        Ok(count) if count > 0 => {
            tracing::info!("Migrated {} agent model IDs to valid OpenRouter slugs", count);
        }
        Err(e) => {
            tracing::warn!("Failed to migrate agent models: {}", e);
        }
        _ => {}
    }

    let name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string();

    let workspace = WorkspaceInfo {
        path: canonical.clone(),
        name,
        project_id: None,
    };

    state.set_workspace(Some(workspace.clone())).await;

    // Write discovery file to the workspace
    let discovery = crate::DiscoveryInfo::new(state.get_port().await, state.get_auth_token().await);
    if let Err(e) = discovery.write_to_workspace(&canonical).await {
        tracing::warn!("Failed to write discovery file: {}", e);
    }

    // Load agents from workspace and update state
    let paths = studio_storage::ReactorPaths::new(&canonical);
    let loader = studio_storage::AgentLoader::new(paths);
    if let Ok(agents) = loader.list_all() {
        let agent_infos: Vec<crate::state::AgentInfo> = agents
            .into_iter()
            .map(|a| crate::state::AgentInfo {
                id: a.id,
                name: a.name,
                color: a.color,
                icon: a.icon,
            })
            .collect();
        state.set_agents(agent_infos).await;
        
        // Select first agent by default
        if let Some(first_agent) = state.get_agents().await.first() {
            state.set_selected_agent(Some(first_agent.id.clone())).await;
        }
    }

    (
        StatusCode::OK,
        Json(OpenWorkspaceResponse {
            success: true,
            workspace: Some(workspace),
            error: None,
        }),
    )
}
