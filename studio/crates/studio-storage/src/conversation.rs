// Ported from 1jehuang/jcode (MIT) - jcode-storage/src/conversation.rs
// Adapted for Reactor Studio.

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

use studio_protocol::{ConversationId, Message, AgentId};

use crate::{ReactorPaths, StorageError};

/// Metadata stored at the start of each conversation file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMeta {
    #[serde(rename = "$meta")]
    pub meta: ConversationMetaInner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMetaInner {
    pub agent_id: String,
    pub title: String,
    pub created: chrono::DateTime<chrono::Utc>,
    pub updated: chrono::DateTime<chrono::Utc>,
}

/// Summary of a conversation for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub agent_id: String,
    pub title: String,
    pub created: chrono::DateTime<chrono::Utc>,
    pub updated: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
}

/// Store for conversation files
pub struct ConversationStore {
    paths: ReactorPaths,
}

impl ConversationStore {
    pub fn new(paths: ReactorPaths) -> Self {
        Self { paths }
    }

    /// Create a new conversation
    pub fn create(
        &self,
        agent_id: &AgentId,
        title: Option<String>,
    ) -> Result<ConversationId, StorageError> {
        let id = ConversationId::new();
        let now = chrono::Utc::now();

        let meta = ConversationMeta {
            meta: ConversationMetaInner {
                agent_id: agent_id.as_str().to_string(),
                title: title.unwrap_or_else(|| format!("Conversation {}", now.format("%Y-%m-%d %H:%M"))),
                created: now,
                updated: now,
            },
        };

        let file_path = self.paths.conversation_file(id.as_str());
        std::fs::create_dir_all(file_path.parent().unwrap())?;

        let mut file = File::create(&file_path)?;
        let meta_json = serde_json::to_string(&meta)?;
        writeln!(file, "{}", meta_json)?;

        Ok(id)
    }

    /// List conversations for an agent
    pub fn list(&self, agent_id: &AgentId) -> Result<Vec<ConversationSummary>, StorageError> {
        let conversations_dir = self.paths.conversations_dir();
        if !conversations_dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();

        for entry in std::fs::read_dir(conversations_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Ok(summary) = self.read_summary(&path) {
                    if summary.agent_id == agent_id.as_str() {
                        summaries.push(summary);
                    }
                }
            }
        }

        summaries.sort_by(|a, b| b.updated.cmp(&a.updated));
        Ok(summaries)
    }

    /// List all conversations across all agents
    pub fn list_all(&self) -> Result<Vec<ConversationSummary>, StorageError> {
        let conversations_dir = self.paths.conversations_dir();
        if !conversations_dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();

        for entry in std::fs::read_dir(conversations_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Ok(summary) = self.read_summary(&path) {
                    summaries.push(summary);
                }
            }
        }

        summaries.sort_by(|a, b| b.updated.cmp(&a.updated));
        Ok(summaries)
    }

    /// Read conversation messages
    pub fn read_messages(&self, conversation_id: &ConversationId) -> Result<Vec<Message>, StorageError> {
        let file_path = self.paths.conversation_file(conversation_id.as_str());
        if !file_path.exists() {
            return Err(StorageError::NotFound(format!(
                "Conversation not found: {}",
                conversation_id.as_str()
            )));
        }

        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            // Skip meta line
            if i == 0 && line.contains("\"$meta\"") {
                continue;
            }

            let message: Message = serde_json::from_str(&line)?;
            messages.push(message);
        }

        Ok(messages)
    }

    /// Append a message to a conversation
    pub fn append_message(
        &self,
        conversation_id: &ConversationId,
        message: &Message,
    ) -> Result<(), StorageError> {
        use std::io::Write;
        
        let file_path = self.paths.conversation_file(conversation_id.as_str());
        if !file_path.exists() {
            return Err(StorageError::NotFound(format!(
                "Conversation not found: {}",
                conversation_id.as_str()
            )));
        }

        let mut file = OpenOptions::new().append(true).open(&file_path)?;
        let message_json = serde_json::to_string(message)?;
        writeln!(file, "{}", message_json)?;
        file.flush()?;

        // Update the meta timestamp
        self.update_timestamp(conversation_id)?;

        Ok(())
    }

    /// Delete a conversation
    pub fn delete(&self, conversation_id: &ConversationId) -> Result<(), StorageError> {
        let file_path = self.paths.conversation_file(conversation_id.as_str());
        if file_path.exists() {
            std::fs::remove_file(file_path)?;
        }
        Ok(())
    }

    fn read_summary(&self, path: &std::path::Path) -> Result<ConversationSummary, StorageError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Read meta from first line
        let first_line = lines.next().ok_or_else(|| {
            StorageError::InvalidFormat("Empty conversation file".to_string())
        })??;

        let meta: ConversationMeta = serde_json::from_str(&first_line)?;

        // Count messages
        let message_count = lines.filter(|l| l.is_ok()).count();

        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(ConversationSummary {
            id,
            agent_id: meta.meta.agent_id,
            title: meta.meta.title,
            created: meta.meta.created,
            updated: meta.meta.updated,
            message_count,
        })
    }

    fn update_timestamp(&self, conversation_id: &ConversationId) -> Result<(), StorageError> {
        let file_path = self.paths.conversation_file(conversation_id.as_str());
        let content = std::fs::read_to_string(&file_path)?;
        let mut lines: Vec<&str> = content.lines().collect();

        if let Some(first_line) = lines.first_mut() {
            if let Ok(mut meta) = serde_json::from_str::<ConversationMeta>(first_line) {
                meta.meta.updated = chrono::Utc::now();
                let new_meta = serde_json::to_string(&meta)?;
                
                let rest = lines[1..].join("\n");
                let new_content = format!("{}\n{}", new_meta, rest);
                std::fs::write(&file_path, new_content)?;
            }
        }

        Ok(())
    }
}
