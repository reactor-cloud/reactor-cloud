// Ported from 1jehuang/jcode (MIT) - src/storage/agent.rs
// Adapted for Reactor Studio.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{ReactorPaths, StorageError};

/// Agent definition loaded from disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub model: String,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(skip)]
    pub system_prompt: String,
}

impl AgentDefinition {
    pub fn from_yaml(yaml: &str, prompt: &str) -> Result<Self, StorageError> {
        let mut def: AgentDefinition = serde_yaml::from_str(yaml)
            .map_err(|e| StorageError::InvalidFormat(format!("Invalid agent YAML: {}", e)))?;
        def.system_prompt = prompt.to_string();
        Ok(def)
    }
}

/// Loader for agent definitions from .reactor/agents/
pub struct AgentLoader {
    paths: ReactorPaths,
}

impl AgentLoader {
    pub fn new(paths: ReactorPaths) -> Self {
        Self { paths }
    }

    /// Load a specific agent by ID
    pub fn load(&self, agent_id: &str) -> Result<AgentDefinition, StorageError> {
        let yaml_path = self.paths.agent_yaml(agent_id);
        let prompt_path = self.paths.agent_prompt(agent_id);

        if !yaml_path.exists() {
            return Err(StorageError::NotFound(format!(
                "Agent not found: {}",
                agent_id
            )));
        }

        let yaml = std::fs::read_to_string(&yaml_path)?;
        let prompt = if prompt_path.exists() {
            std::fs::read_to_string(&prompt_path)?
        } else {
            String::new()
        };

        AgentDefinition::from_yaml(&yaml, &prompt)
    }

    /// List all available agent IDs
    pub fn list_all(&self) -> Result<Vec<AgentDefinition>, StorageError> {
        let agents_dir = self.paths.agents_dir();
        if !agents_dir.exists() {
            return Ok(Vec::new());
        }

        let mut agents = Vec::new();
        for entry in std::fs::read_dir(agents_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let agent_id = entry.file_name().to_string_lossy().to_string();
                if let Ok(agent) = self.load(&agent_id) {
                    agents.push(agent);
                }
            }
        }

        Ok(agents)
    }

    /// Seed default agents from bundled assets
    pub fn seed_defaults(&self, bundled_agents_dir: &Path) -> Result<(), StorageError> {
        let agents_dir = self.paths.agents_dir();
        
        // Check if any agents exist
        if agents_dir.exists() {
            let has_agents = std::fs::read_dir(&agents_dir)?
                .filter_map(|e| e.ok())
                .any(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false));
            
            if has_agents {
                return Ok(());
            }
        }

        // Copy bundled agents
        if bundled_agents_dir.exists() {
            for entry in std::fs::read_dir(bundled_agents_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let agent_name = entry.file_name();
                    let dest_dir = agents_dir.join(&agent_name);
                    std::fs::create_dir_all(&dest_dir)?;

                    // Copy agent.yaml
                    let src_yaml = entry.path().join("agent.yaml");
                    if src_yaml.exists() {
                        std::fs::copy(&src_yaml, dest_dir.join("agent.yaml"))?;
                    }

                    // Copy prompt.md
                    let src_prompt = entry.path().join("prompt.md");
                    if src_prompt.exists() {
                        std::fs::copy(&src_prompt, dest_dir.join("prompt.md"))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Seed inline default agents (no bundled assets required)
    pub fn seed_inline_defaults(&self) -> Result<(), StorageError> {
        let agents_dir = self.paths.agents_dir();
        
        // Check if any agents exist
        if agents_dir.exists() {
            let has_agents = std::fs::read_dir(&agents_dir)?
                .filter_map(|e| e.ok())
                .any(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false));
            
            if has_agents {
                return Ok(());
            }
        }

        // Create coder agent
        let coder_dir = agents_dir.join("coder");
        std::fs::create_dir_all(&coder_dir)?;
        std::fs::write(coder_dir.join("agent.yaml"), CODER_AGENT_YAML)?;
        std::fs::write(coder_dir.join("prompt.md"), CODER_AGENT_PROMPT)?;

        // Create planner agent
        let planner_dir = agents_dir.join("planner");
        std::fs::create_dir_all(&planner_dir)?;
        std::fs::write(planner_dir.join("agent.yaml"), PLANNER_AGENT_YAML)?;
        std::fs::write(planner_dir.join("prompt.md"), PLANNER_AGENT_PROMPT)?;

        // Create researcher agent
        let researcher_dir = agents_dir.join("researcher");
        std::fs::create_dir_all(&researcher_dir)?;
        std::fs::write(researcher_dir.join("agent.yaml"), RESEARCHER_AGENT_YAML)?;
        std::fs::write(researcher_dir.join("prompt.md"), RESEARCHER_AGENT_PROMPT)?;

        Ok(())
    }

    /// Migrate agent model ids from old invalid values to valid OpenRouter slugs.
    /// Returns the number of agent files updated.
    pub fn migrate_models(&self) -> Result<usize, StorageError> {
        const OLD_MODEL: &str = "anthropic/claude-sonnet-4-20250514";
        const NEW_MODEL: &str = "anthropic/claude-sonnet-4.5";

        let agents_dir = self.paths.agents_dir();
        if !agents_dir.exists() {
            return Ok(0);
        }

        let mut updated_count = 0;

        for entry in std::fs::read_dir(&agents_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let yaml_path = entry.path().join("agent.yaml");
            if !yaml_path.exists() {
                continue;
            }

            let content = std::fs::read_to_string(&yaml_path)?;
            
            // Check if this file has the old model string
            let old_model_line = format!("model: {}", OLD_MODEL);
            if content.contains(&old_model_line) {
                let new_model_line = format!("model: {}", NEW_MODEL);
                let updated_content = content.replace(&old_model_line, &new_model_line);
                std::fs::write(&yaml_path, updated_content)?;
                updated_count += 1;
            }
        }

        Ok(updated_count)
    }
}

const CODER_AGENT_YAML: &str = r##"id: coder
name: Coder
color: "#10b981"
icon: code
model: anthropic/claude-sonnet-4.5
allowed_tools: ["file_read", "file_write", "file_edit", "grep", "glob", "bash"]
"##;

const CODER_AGENT_PROMPT: &str = r#"You are an expert software engineer with deep knowledge of many programming languages, frameworks, and best practices.

Your role is to help the user with coding tasks including:
- Writing new code and implementing features
- Debugging and fixing issues  
- Refactoring and improving existing code
- Explaining code and concepts
- Reviewing code for potential issues

When working with code:
1. Analyze the codebase first using grep and glob tools to understand the project structure
2. Read relevant files to understand context before making changes
3. Make targeted, minimal changes that solve the problem
4. Write clean, idiomatic code following the project's existing style
5. Test your changes when possible

Available tools:
- file_read: Read file contents
- file_write: Write or create files
- file_edit: Make targeted edits to files
- grep: Search for patterns in code
- glob: Find files matching patterns
- bash: Execute shell commands

Always explain your reasoning and approach. When making changes, show the user what you're doing and why.
"#;

const PLANNER_AGENT_YAML: &str = r##"id: planner
name: Planner
color: "#8b5cf6"
icon: lightbulb
model: anthropic/claude-sonnet-4.5
allowed_tools: ["file_read", "grep", "glob", "task_advance", "task_artifact_write"]
"##;

const PLANNER_AGENT_PROMPT: &str = r#"You are an expert technical planner who helps break down complex software tasks into clear, actionable plans.

Your role is to guide tasks through the phased workflow:

## Phase 1: Alignment
Goal: Understand requirements completely before planning.

- Ask clarifying questions to understand the full scope
- Identify edge cases, constraints, and requirements
- Document acceptance criteria with the user
- Confirm understanding before moving forward

When alignment is complete, call `task_advance` with a summary of the agreed requirements.

## Phase 2: Planning
Goal: Create a detailed, actionable implementation plan.

- Analyze the codebase using grep/glob/file_read tools
- Create a structured plan with numbered steps
- Break down work into small, testable increments
- Identify files that need changes
- Document testing strategy and rollback plan

When planning is complete:
1. Call `task_artifact_write` with artifact_type="plan" to save the plan
2. Call `task_advance` with a summary to move to Development

## Phase 3+: Development/Testing/UAT/Deployment
These phases are handled by the Coder agent.

## Available Tools
- file_read: Read file contents to understand existing code
- grep: Search for patterns to find relevant code
- glob: Find files matching patterns
- task_advance: Signal that current phase is complete and ready to advance
- task_artifact_write: Write plan or report artifacts for the task

## Guidelines
- Always understand before prescribing solutions
- Ask questions when requirements are ambiguous
- Be thorough in planning to reduce rework during development
- Document your reasoning in the plan artifact
"#;

const RESEARCHER_AGENT_YAML: &str = r##"id: researcher
name: Researcher
color: "#f59e0b"
icon: search
model: anthropic/claude-sonnet-4.5
allowed_tools: ["file_read", "grep", "glob", "bash"]
"##;

const RESEARCHER_AGENT_PROMPT: &str = r#"You are an expert code researcher who helps developers understand and navigate codebases.

Your role is to:
- Answer questions about how code works
- Find relevant code and documentation
- Trace data flow and dependencies
- Explain architectural patterns and design decisions
- Help developers get oriented in unfamiliar codebases

Research strategies:
1. Start with grep/glob to find relevant files
2. Read key files to understand structure
3. Trace imports and dependencies
4. Look for tests to understand intended behavior
5. Check documentation and comments

Available tools:
- file_read: Read file contents
- grep: Search for patterns in code
- glob: Find files matching patterns
- bash: Run commands (e.g., to check dependencies, git history)

Provide clear, well-organized summaries of your findings. Cite specific files and line numbers.
"#;

// YAML support
mod serde_yaml {
    use serde::de::DeserializeOwned;

    pub fn from_str<T: DeserializeOwned>(s: &str) -> Result<T, String> {
        // Simple YAML parser - just handle the basic key: value format
        // For a production version, use the `serde_yaml` crate
        let json = yaml_to_json(s)?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    fn yaml_to_json(yaml: &str) -> Result<String, String> {
        let mut json = String::from("{");
        let mut first = true;

        for line in yaml.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                if !first {
                    json.push(',');
                }
                first = false;

                json.push('"');
                json.push_str(key);
                json.push_str("\":");

                if value.starts_with('[') {
                    // Array value
                    json.push_str(value);
                } else if value.starts_with('"') {
                    json.push_str(value);
                } else if value.is_empty() {
                    json.push_str("null");
                } else if value == "true" || value == "false" {
                    json.push_str(value);
                } else if value.parse::<f64>().is_ok() {
                    json.push_str(value);
                } else {
                    json.push('"');
                    json.push_str(value);
                    json.push('"');
                }
            }
        }

        json.push('}');
        Ok(json)
    }
}
