use crate::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, State, Window};
#[cfg(feature = "devserver")]
use tauri::Manager;
use studio_storage::{ReactorPaths, AgentLoader};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub project_id: String,
    pub project_name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    pub workspace_path: Option<String>,
    pub selected_agent_id: Option<String>,
    pub active_conversation_id: Option<String>,
    pub file_browser_open: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("Path does not exist: {0}")]
    PathNotFound(String),
    #[error("Path is not a directory: {0}")]
    NotADirectory(String),
    #[error("Failed to create .reactor directory: {0}")]
    CreateDirFailed(String),
    #[error("Failed to write config: {0}")]
    WriteConfigFailed(String),
    #[error("Workspace already open in another window")]
    AlreadyOpen,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for WorkspaceError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
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

fn scaffold_reactor_dir(path: &PathBuf) -> Result<(), WorkspaceError> {
    let reactor_dir = path.join(".reactor");

    if !reactor_dir.exists() {
        fs::create_dir_all(&reactor_dir)
            .map_err(|e| WorkspaceError::CreateDirFailed(e.to_string()))?;

        // Create subdirectories
        fs::create_dir_all(reactor_dir.join("agents"))?;
        fs::create_dir_all(reactor_dir.join("conversations"))?;
        fs::create_dir_all(reactor_dir.join("tasks"))?;
        fs::create_dir_all(reactor_dir.join("memory"))?;
        fs::create_dir_all(reactor_dir.join("cache"))?;

        // Create config.toml
        let project_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");

        let config_path = reactor_dir.join("config.toml");
        fs::write(&config_path, create_default_config(project_name))
            .map_err(|e| WorkspaceError::WriteConfigFailed(e.to_string()))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn workspace_open(
    path: String,
    window: Window,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<WorkspaceInfo, WorkspaceError> {
    let path_buf = PathBuf::from(&path);

    // Verify path exists and is a directory
    if !path_buf.exists() {
        return Err(WorkspaceError::PathNotFound(path.clone()));
    }
    if !path_buf.is_dir() {
        return Err(WorkspaceError::NotADirectory(path.clone()));
    }

    // Canonicalize path for comparison
    let canonical = path_buf.canonicalize()?;

    // Check if already open
    {
        let workspaces = state.workspaces.lock().unwrap();
        if let Some(existing_label) = workspaces.get(&canonical) {
            if existing_label != window.label() {
                return Err(WorkspaceError::AlreadyOpen);
            }
        }
    }

    // Scaffold .reactor/ if needed
    scaffold_reactor_dir(&canonical)?;

    // Seed default agents
    let paths = ReactorPaths::new(&canonical);
    let agent_loader = AgentLoader::new(paths);
    if let Err(e) = agent_loader.seed_inline_defaults() {
        eprintln!("Failed to seed default agents: {}", e);
    }

    // Migrate old model IDs to valid OpenRouter slugs
    match agent_loader.migrate_models() {
        Ok(count) if count > 0 => {
            eprintln!("Migrated {} agent model IDs to valid OpenRouter slugs", count);
        }
        Err(e) => {
            eprintln!("Failed to migrate agent models: {}", e);
        }
        _ => {}
    }

    // Write devserver discovery file if devserver is running
    #[cfg(feature = "devserver")]
    {
        use crate::DevServerState;
        if let Some(ds_state) = app_handle.try_state::<DevServerState>() {
            if let Err(e) = ds_state.write_discovery(&canonical).await {
                eprintln!("Failed to write devserver discovery file: {}", e);
            } else {
                eprintln!("Wrote devserver discovery file to {:?}", canonical.join(".reactor/dev-server.json"));
            }
        }
    }

    // Register this workspace
    {
        let mut workspaces = state.workspaces.lock().unwrap();
        workspaces.insert(canonical.clone(), window.label().to_string());
    }

    let project_name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    // Generate a simple project ID
    let project_id = format!(
        "proj_{}",
        &sha256_simple(&canonical.to_string_lossy())[..12]
    );

    Ok(WorkspaceInfo {
        project_id,
        project_name,
        path: canonical.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn workspace_state() -> WorkspaceState {
    WorkspaceState {
        workspace_path: None,
        selected_agent_id: Some("coder".to_string()),
        active_conversation_id: None,
        file_browser_open: true,
    }
}

fn sha256_simple(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
