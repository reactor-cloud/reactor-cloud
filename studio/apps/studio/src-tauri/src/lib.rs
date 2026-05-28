use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

mod ipc;

#[cfg(feature = "devserver")]
use std::sync::Arc;
#[cfg(feature = "devserver")]
use tokio::sync::RwLock;

#[cfg(feature = "foundry")]
use ipc::foundry::FoundryState;

pub struct AppState {
    /// Map from canonical workspace path to window label
    pub workspaces: Mutex<HashMap<PathBuf, String>>,
    /// Current workspace path for devserver
    #[cfg(feature = "devserver")]
    pub current_workspace: Arc<RwLock<Option<PathBuf>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            workspaces: Mutex::new(HashMap::new()),
            #[cfg(feature = "devserver")]
            current_workspace: Arc::new(RwLock::new(None)),
        }
    }
}

/// State for the devserver (port, token, etc.)
#[cfg(feature = "devserver")]
pub struct DevServerState {
    pub port: u16,
    pub token: String,
}

#[cfg(feature = "devserver")]
impl DevServerState {
    pub fn new(port: u16, token: String) -> Self {
        Self { port, token }
    }

    /// Write discovery file to the given workspace path
    pub async fn write_discovery(&self, workspace_path: &std::path::Path) -> std::io::Result<()> {
        let discovery = studio_devserver::DiscoveryInfo::new(self.port, self.token.clone());
        discovery.write_to_workspace(workspace_path).await
    }
}

/// Check if devserver should be started
#[cfg(feature = "devserver")]
fn should_start_devserver() -> bool {
    // Always start in dev builds, or when env var is set
    cfg!(debug_assertions) || std::env::var("REACTOR_STUDIO_DEVSERVER").is_ok()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState::default())
        .manage(ipc::agent::AgentState::default())
        .manage(ipc::trace::TraceState::default());

    #[cfg(feature = "foundry")]
    {
        // Initialize foundry state with path relative to workspace
        let foundry_path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".foundry");
        builder = builder.manage(FoundryState::new(foundry_path));
    }

    let builder = builder.invoke_handler(tauri::generate_handler![
        ipc::workspace::workspace_open,
        ipc::workspace::workspace_state,
        ipc::files::file_read,
        ipc::files::file_write,
        ipc::files::file_list,
        ipc::window::window_get_state,
        ipc::window::window_set_state,
        ipc::agent::agent_list,
        ipc::agent::agent_send,
        ipc::agent::agent_cancel,
        ipc::agent::tool_approve,
        ipc::credentials::credential_set,
        ipc::credentials::credential_get,
        ipc::credentials::credential_delete,
        ipc::credentials::credential_check,
        ipc::credentials::credential_list,
        ipc::conversation::conversation_list,
        ipc::conversation::conversation_list_all,
        ipc::conversation::conversation_create,
        ipc::conversation::conversation_messages,
        ipc::conversation::conversation_delete,
        ipc::task::task_list,
        ipc::task::task_create,
        ipc::task::task_get,
        ipc::task::task_advance,
        ipc::task::task_phase_messages,
        ipc::task::task_delete,
        ipc::trace::trace_get,
        ipc::trace::trace_list_conversations,
        // Foundry commands (feature-gated)
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_baseline,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_run,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_stop,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_status,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_replay_record,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_replay_play,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_lessons_list,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_lessons_show,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_lessons_stats,
        #[cfg(feature = "foundry")]
        ipc::foundry::foundry_reports_list,
    ]);

    #[cfg(feature = "devserver")]
    let builder = builder.setup(|app| {
        if should_start_devserver() {
            let app_handle = app.handle().clone();
            let app_handle_for_state = app.handle().clone();

            // Spawn devserver startup task
            tauri::async_runtime::spawn(async move {
                // Start devserver with app handle for Tauri integration
                match studio_devserver::DevServer::start_with_app_handle(None, app_handle).await {
                    Ok(server) => {
                        let port = server.port();
                        let token = server.discovery().token.clone();
                        
                        tracing::info!(
                            "DevServer started on http://127.0.0.1:{} (token: {}...)",
                            port,
                            &token[..20]
                        );

                        // Store devserver state for later use (writing discovery file)
                        app_handle_for_state.manage(DevServerState::new(port, token));
                    }
                    Err(e) => {
                        tracing::error!("Failed to start DevServer: {}", e);
                    }
                }
            });
        }
        Ok(())
    });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
