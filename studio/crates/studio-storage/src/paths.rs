// Ported from 1jehuang/jcode (MIT) - jcode-storage/src/paths.rs
// Adapted for Reactor Studio.

use std::path::{Path, PathBuf};

/// Helper for resolving all .reactor/* paths from project root
#[derive(Clone)]
pub struct ReactorPaths {
    root: PathBuf,
}

impl ReactorPaths {
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        Self {
            root: project_root.as_ref().to_path_buf(),
        }
    }

    pub fn project_root(&self) -> &Path {
        &self.root
    }

    pub fn reactor_dir(&self) -> PathBuf {
        self.root.join(".reactor")
    }

    pub fn config_file(&self) -> PathBuf {
        self.reactor_dir().join("config.toml")
    }

    pub fn agents_dir(&self) -> PathBuf {
        self.reactor_dir().join("agents")
    }

    pub fn agent_dir(&self, agent_id: &str) -> PathBuf {
        self.agents_dir().join(agent_id)
    }

    pub fn agent_yaml(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("agent.yaml")
    }

    pub fn agent_prompt(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("prompt.md")
    }

    pub fn conversations_dir(&self) -> PathBuf {
        self.reactor_dir().join("conversations")
    }

    pub fn conversation_file(&self, conversation_id: &str) -> PathBuf {
        self.conversations_dir().join(format!("{}.jsonl", conversation_id))
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.reactor_dir().join("tasks")
    }

    pub fn task_dir(&self, task_id: &str) -> PathBuf {
        self.tasks_dir().join(task_id)
    }

    pub fn memory_dir(&self) -> PathBuf {
        self.reactor_dir().join("memory")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.reactor_dir().join("cache")
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.reactor_dir())?;
        std::fs::create_dir_all(self.agents_dir())?;
        std::fs::create_dir_all(self.conversations_dir())?;
        std::fs::create_dir_all(self.tasks_dir())?;
        std::fs::create_dir_all(self.memory_dir())?;
        std::fs::create_dir_all(self.cache_dir())?;
        Ok(())
    }
}
