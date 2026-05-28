// Ported from 1jehuang/jcode (MIT) - jcode-memory
// Adapted for Reactor Studio.
// 
// This is a minimal v0 implementation - just conversation history retrieval.
// Full jcode memory graph is deferred to Phase 5+.

use studio_protocol::{ConversationId, Message};
use studio_storage::{ConversationStore, ReactorPaths, StorageError};

/// Memory manager for conversation context
pub struct MemoryManager {
    conversation_store: ConversationStore,
}

impl MemoryManager {
    pub fn new(paths: ReactorPaths) -> Self {
        Self {
            conversation_store: ConversationStore::new(paths),
        }
    }

    /// Get conversation history for context
    pub fn get_history(
        &self,
        conversation_id: &ConversationId,
        max_messages: Option<usize>,
    ) -> Result<Vec<Message>, StorageError> {
        let messages = self.conversation_store.read_messages(conversation_id)?;
        
        if let Some(max) = max_messages {
            let len = messages.len();
            if len > max {
                return Ok(messages.into_iter().skip(len - max).collect());
            }
        }
        
        Ok(messages)
    }

    /// Get the most recent messages
    pub fn get_recent_messages(
        &self,
        conversation_id: &ConversationId,
        count: usize,
    ) -> Result<Vec<Message>, StorageError> {
        self.get_history(conversation_id, Some(count))
    }

    /// Save a message to history
    pub fn save_message(
        &self,
        conversation_id: &ConversationId,
        message: &Message,
    ) -> Result<(), StorageError> {
        self.conversation_store.append_message(conversation_id, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_memory_manager_creation() {
        let paths = ReactorPaths::new(PathBuf::from("/tmp/test"));
        let _manager = MemoryManager::new(paths);
    }
}
