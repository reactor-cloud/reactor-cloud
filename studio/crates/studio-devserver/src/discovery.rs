use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
    pub version: String,
    pub started_at: String,
}

impl DiscoveryInfo {
    pub fn new(port: u16, token: String) -> Self {
        Self {
            port,
            token,
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub async fn write_to_workspace(&self, workspace_path: &Path) -> std::io::Result<()> {
        let reactor_dir = workspace_path.join(".reactor");
        fs::create_dir_all(&reactor_dir).await?;

        let discovery_path = reactor_dir.join("dev-server.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&discovery_path, json).await?;

        tracing::info!("Discovery file written to {:?}", discovery_path);
        Ok(())
    }

    pub async fn read_from_workspace(workspace_path: &Path) -> std::io::Result<Self> {
        let discovery_path = workspace_path.join(".reactor/dev-server.json");
        let content = fs::read_to_string(&discovery_path).await?;
        let info: Self = serde_json::from_str(&content)?;
        Ok(info)
    }

    pub async fn remove_from_workspace(workspace_path: &Path) -> std::io::Result<()> {
        let discovery_path = workspace_path.join(".reactor/dev-server.json");
        if discovery_path.exists() {
            fs::remove_file(&discovery_path).await?;
        }
        Ok(())
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}
