use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use chrono::Utc;
use studio_protocol::Message;

use crate::{Phase, PhaseStatus, Task, TaskError, TaskId, TaskSummary, TaskState};

/// File-backed store for tasks
pub struct TaskStore {
    tasks_dir: PathBuf,
}

impl TaskStore {
    pub fn new(reactor_path: impl Into<PathBuf>) -> Self {
        let reactor_path: PathBuf = reactor_path.into();
        Self {
            tasks_dir: reactor_path.join("tasks"),
        }
    }

    fn task_dir(&self, task_id: &TaskId) -> PathBuf {
        self.tasks_dir.join(task_id.as_str())
    }

    fn task_meta_path(&self, task_id: &TaskId) -> PathBuf {
        self.task_dir(task_id).join("task.json")
    }

    fn phase_messages_path(&self, task_id: &TaskId, phase: Phase) -> PathBuf {
        self.task_dir(task_id)
            .join(format!("{}.jsonl", phase.name().to_lowercase()))
    }

    /// Create a new task
    pub fn create(&self, title: &str, description: Option<&str>) -> Result<Task, TaskError> {
        let mut task = Task::new(title);
        if let Some(desc) = description {
            task = task.with_description(desc);
        }

        // Create task directory
        let task_dir = self.task_dir(&task.id);
        fs::create_dir_all(&task_dir)?;

        // Write task metadata
        self.save_task(&task)?;

        // Create initial phase conversation file
        let phase_path = self.phase_messages_path(&task.id, Phase::Alignment);
        File::create(&phase_path)?;

        Ok(task)
    }

    /// List all tasks
    pub fn list(&self) -> Result<Vec<TaskSummary>, TaskError> {
        if !self.tasks_dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();

        for entry in fs::read_dir(&self.tasks_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let task_id = TaskId::from_string(&entry.file_name().to_string_lossy());
                if let Ok(task) = self.get(&task_id) {
                    summaries.push(TaskSummary::from(&task));
                }
            }
        }

        // Sort by updated_at descending
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(summaries)
    }

    /// Get a task by ID
    pub fn get(&self, task_id: &TaskId) -> Result<Task, TaskError> {
        let meta_path = self.task_meta_path(task_id);
        if !meta_path.exists() {
            return Err(TaskError::NotFound(task_id.as_str().to_string()));
        }

        let content = fs::read_to_string(&meta_path)?;
        let task: Task = serde_json::from_str(&content)?;
        Ok(task)
    }

    /// Advance task to the next phase
    pub fn advance(&self, task_id: &TaskId) -> Result<Task, TaskError> {
        let mut task = self.get(task_id)?;

        if !task.can_advance() {
            return Err(TaskError::PhaseNotReady(
                task.current_phase.name().to_string(),
            ));
        }

        let next_phase = task.current_phase.next().ok_or_else(|| {
            TaskError::InvalidTransition {
                from: task.current_phase.name().to_string(),
                to: "none".to_string(),
            }
        })?;

        // Mark current phase as completed
        task.current_phase_state_mut().status = PhaseStatus::Completed;
        task.current_phase_state_mut().completed_at = Some(Utc::now());

        // Move to next phase
        task.current_phase = next_phase;
        task.phases[next_phase.index()].status = PhaseStatus::Active;
        task.phases[next_phase.index()].started_at = Some(Utc::now());
        task.updated_at = Utc::now();

        // Create phase conversation file if needed
        let phase_path = self.phase_messages_path(task_id, next_phase);
        if !phase_path.exists() {
            File::create(&phase_path)?;
        }

        // If this was the last phase, mark task as completed
        if next_phase == Phase::Deployment {
            // Deployment completes immediately (can be refined later)
        }

        self.save_task(&task)?;
        Ok(task)
    }

    /// Complete the current task
    pub fn complete(&self, task_id: &TaskId) -> Result<Task, TaskError> {
        let mut task = self.get(task_id)?;
        task.state = TaskState::Completed;
        task.current_phase_state_mut().status = PhaseStatus::Completed;
        task.current_phase_state_mut().completed_at = Some(Utc::now());
        task.updated_at = Utc::now();
        self.save_task(&task)?;
        Ok(task)
    }

    /// Append a message to a phase conversation
    pub fn append_message(
        &self,
        task_id: &TaskId,
        phase: Phase,
        message: &Message,
    ) -> Result<(), TaskError> {
        let path = self.phase_messages_path(task_id, phase);

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        let json = serde_json::to_string(message)?;
        writeln!(file, "{}", json)?;

        // Update task timestamp
        let mut task = self.get(task_id)?;
        task.updated_at = Utc::now();
        self.save_task(&task)?;

        Ok(())
    }

    /// Get messages for a phase
    pub fn phase_messages(&self, task_id: &TaskId, phase: Phase) -> Result<Vec<Message>, TaskError> {
        let path = self.phase_messages_path(task_id, phase);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let message: Message = serde_json::from_str(&line)?;
            messages.push(message);
        }

        Ok(messages)
    }

    /// Delete a task
    pub fn delete(&self, task_id: &TaskId) -> Result<(), TaskError> {
        let task_dir = self.task_dir(task_id);
        if task_dir.exists() {
            fs::remove_dir_all(&task_dir)?;
        }
        Ok(())
    }

    fn save_task(&self, task: &Task) -> Result<(), TaskError> {
        let meta_path = self.task_meta_path(&task.id);
        let content = serde_json::to_string_pretty(task)?;
        fs::write(&meta_path, content)?;
        Ok(())
    }
}
