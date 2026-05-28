use std::fs;
use tempfile::tempdir;

use studio_protocol::Message;
use studio_task::{Phase, PhaseStatus, TaskStore, TaskState};

#[test]
fn test_task_creation() {
    let temp = tempdir().unwrap();
    let reactor_path = temp.path().join(".reactor");
    fs::create_dir_all(&reactor_path).unwrap();

    let store = TaskStore::new(&reactor_path);
    let task = store.create("Test feature", Some("Build a new feature")).unwrap();

    assert_eq!(task.title, "Test feature");
    assert_eq!(task.description, "Build a new feature");
    assert_eq!(task.current_phase, Phase::Alignment);
    assert_eq!(task.state, TaskState::Active);
}

#[test]
fn test_task_phase_advancement() {
    let temp = tempdir().unwrap();
    let reactor_path = temp.path().join(".reactor");
    fs::create_dir_all(&reactor_path).unwrap();

    let store = TaskStore::new(&reactor_path);
    let task = store.create("Test task", None).unwrap();

    // Initial state
    assert_eq!(task.current_phase, Phase::Alignment);
    assert_eq!(task.phases[0].status, PhaseStatus::Active);
    assert_eq!(task.phases[1].status, PhaseStatus::Locked);

    // Advance to Planning
    let task = store.advance(&task.id).unwrap();
    assert_eq!(task.current_phase, Phase::Planning);
    assert_eq!(task.phases[0].status, PhaseStatus::Completed);
    assert_eq!(task.phases[1].status, PhaseStatus::Active);
    assert_eq!(task.phases[2].status, PhaseStatus::Locked);

    // Advance to Development
    let task = store.advance(&task.id).unwrap();
    assert_eq!(task.current_phase, Phase::Development);
    assert_eq!(task.phases[1].status, PhaseStatus::Completed);
    assert_eq!(task.phases[2].status, PhaseStatus::Active);
}

#[test]
fn test_task_list() {
    let temp = tempdir().unwrap();
    let reactor_path = temp.path().join(".reactor");
    fs::create_dir_all(&reactor_path).unwrap();

    let store = TaskStore::new(&reactor_path);
    
    store.create("Task 1", None).unwrap();
    store.create("Task 2", None).unwrap();
    store.create("Task 3", None).unwrap();

    let tasks = store.list().unwrap();
    assert_eq!(tasks.len(), 3);
}

#[test]
fn test_phase_messages() {
    let temp = tempdir().unwrap();
    let reactor_path = temp.path().join(".reactor");
    fs::create_dir_all(&reactor_path).unwrap();

    let store = TaskStore::new(&reactor_path);
    let task = store.create("Message test", None).unwrap();

    // Append messages to alignment phase
    let msg1 = Message::user("What should this feature do?");
    let msg2 = Message::assistant("I'll need to understand the requirements...");

    store.append_message(&task.id, Phase::Alignment, &msg1).unwrap();
    store.append_message(&task.id, Phase::Alignment, &msg2).unwrap();

    // Read messages back
    let messages = store.phase_messages(&task.id, Phase::Alignment).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "What should this feature do?");
}

#[test]
fn test_task_delete() {
    let temp = tempdir().unwrap();
    let reactor_path = temp.path().join(".reactor");
    fs::create_dir_all(&reactor_path).unwrap();

    let store = TaskStore::new(&reactor_path);
    let task = store.create("Delete me", None).unwrap();

    // Verify task exists
    assert!(store.get(&task.id).is_ok());

    // Delete task
    store.delete(&task.id).unwrap();

    // Verify task is gone
    assert!(store.get(&task.id).is_err());
}
