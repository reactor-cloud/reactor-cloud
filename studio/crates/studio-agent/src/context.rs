// Ported from 1jehuang/jcode (MIT) - src/agent/context.rs
// Adapted for Reactor Studio.

use studio_protocol::Message;
use studio_storage::AgentDefinition;

/// Builder for constructing the context (system prompt + conversation history)
pub struct ContextBuilder {
    workspace_path: String,
    workspace_name: String,
}

impl ContextBuilder {
    pub fn new(workspace_path: impl Into<String>, workspace_name: impl Into<String>) -> Self {
        Self {
            workspace_path: workspace_path.into(),
            workspace_name: workspace_name.into(),
        }
    }

    /// Build the full context for an agent
    pub fn build(&self, agent: &AgentDefinition, history: &[Message], user_query: &str) -> Vec<Message> {
        let mut messages = Vec::new();

        // System message with XML-style sections
        let system_content = self.build_system_prompt(agent);
        messages.push(Message::system(system_content));

        // Add conversation history
        for msg in history {
            messages.push(msg.clone());
        }

        // Add user query
        messages.push(Message::user(user_query));

        messages
    }

    fn build_system_prompt(&self, agent: &AgentDefinition) -> String {
        let mut prompt = String::new();

        // Agent-specific system prompt
        prompt.push_str("<system>\n");
        prompt.push_str(&agent.system_prompt);
        prompt.push_str("\n</system>\n\n");

        // Workspace context
        prompt.push_str("<workspace_context>\n");
        prompt.push_str(&format!("Workspace: {}\n", self.workspace_name));
        prompt.push_str(&format!("Path: {}\n", self.workspace_path));
        prompt.push_str("</workspace_context>\n\n");

        // Agent context
        prompt.push_str("<agent_context>\n");
        prompt.push_str(&format!("Agent: {} ({})\n", agent.name, agent.id));
        prompt.push_str(&format!("Model: {}\n", agent.model));
        if !agent.allowed_tools.is_empty() {
            prompt.push_str(&format!("Available tools: {}\n", agent.allowed_tools.join(", ")));
        }
        prompt.push_str("</agent_context>\n");

        prompt
    }

    /// Build context with prior task phase summaries
    pub fn build_with_task_context(
        &self,
        agent: &AgentDefinition,
        history: &[Message],
        user_query: &str,
        task_context: &str,
    ) -> Vec<Message> {
        let mut messages = Vec::new();

        // System message with task context
        let mut system_content = self.build_system_prompt(agent);
        
        system_content.push_str("\n<task_context>\n");
        system_content.push_str(task_context);
        system_content.push_str("\n</task_context>\n");

        messages.push(Message::system(system_content));

        // Add conversation history
        for msg in history {
            messages.push(msg.clone());
        }

        // Add user query
        messages.push(Message::user(user_query));

        messages
    }
}
