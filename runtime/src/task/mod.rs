//! # Task Manager
//!
//! Creates, tracks, and manages task lifecycle.  Each task
//! belongs to an optional session and carries a context.
//!
//! ## State diagram
//!
//! ```text
//! Pending ──→ Running ──→ Completed
//!                │
//!                ├──→ Failed
//!                └──→ Cancelled
//! ```
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::context::Context;
use crate::error::RuntimeError;
use crate::session::SessionId;

// ── Task ID ───────────────────────────────────────────────

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Task state ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Failed | TaskState::Cancelled)
    }
}

// ── Task struct ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub session_id: Option<SessionId>,
    pub state: TaskState,
    pub context: Context,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

// ── Task priority ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(u8);

impl Priority {
    pub const LOW: Priority = Priority(100);
    pub const NORMAL: Priority = Priority(50);
    pub const HIGH: Priority = Priority(10);
    pub const CRITICAL: Priority = Priority(0);

    pub fn new(level: u8) -> Self {
        Priority(level)
    }
}

// ── Task handle (for scheduler interactions) ─────────────

/// A lightweight reference to a task used by the scheduler.
#[derive(Debug, Clone)]
pub struct TaskHandle {
    pub task_id: TaskId,
    pub priority: Priority,
    pub session_id: Option<SessionId>,
    pub created_at: DateTime<Utc>,
}

// ── Task manager trait ───────────────────────────────────

/// Manages task creation, state, and queries.
pub trait TaskManager: Debug + Send + Sync {
    fn create_task(
        &self,
        session_id: Option<SessionId>,
        priority: Priority,
        metadata: HashMap<String, String>,
    ) -> Task;

    fn get_task(&self, id: &TaskId) -> Option<Task>;

    fn update_state(&self, id: &TaskId, state: TaskState) -> Result<(), RuntimeError>;

    fn list_tasks(&self) -> Vec<Task>;

    fn list_tasks_by_session(&self, session_id: &SessionId) -> Vec<Task>;

    fn task_count(&self) -> usize;

    fn pending_count(&self) -> usize;
}

// ── Implementation ───────────────────────────────────────

#[derive(Debug)]
pub struct DefaultTaskManager {
    tasks: RwLock<HashMap<TaskId, Task>>,
    counter: AtomicU64,
}

impl DefaultTaskManager {
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }
    }
}

impl Default for DefaultTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager for DefaultTaskManager {
    fn create_task(
        &self,
        session_id: Option<SessionId>,
        priority: Priority,
        metadata: HashMap<String, String>,
    ) -> Task {
        let id = TaskId::new();
        self.counter.fetch_add(1, Ordering::Relaxed);
        let task = Task {
            id: id.clone(),
            session_id,
            state: TaskState::Pending,
            context: Context::new().with_metadata("priority", &priority.0.to_string()),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            metadata,
        };
        if let Ok(mut guard) = self.tasks.write() {
            guard.insert(id, task.clone());
        }
        task
    }

    fn get_task(&self, id: &TaskId) -> Option<Task> {
        self.tasks.read().ok()?.get(id).cloned()
    }

    fn update_state(&self, id: &TaskId, state: TaskState) -> Result<(), RuntimeError> {
        let mut guard = self.tasks.write().map_err(|_| {
            RuntimeError::Task("lock poisoned".into())
        })?;
        let task = guard.get_mut(id).ok_or_else(|| {
            RuntimeError::Task(format!("task {} not found", id))
        })?;

        let now = Utc::now();
        task.state = state;
        match task.state {
            TaskState::Running => task.started_at = Some(now),
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled => {
                task.completed_at = Some(now);
            }
            _ => {}
        }
        Ok(())
    }

    fn list_tasks(&self) -> Vec<Task> {
        self.tasks
            .read()
            .map(|g| {
                let mut tasks: Vec<_> = g.values().cloned().collect();
                tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                tasks
            })
            .unwrap_or_default()
    }

    fn list_tasks_by_session(&self, session_id: &SessionId) -> Vec<Task> {
        self.tasks
            .read()
            .map(|g| {
                g.values()
                    .filter(|t| t.session_id.as_ref() == Some(session_id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn task_count(&self) -> usize {
        self.tasks.read().map(|g| g.len()).unwrap_or(0)
    }

    fn pending_count(&self) -> usize {
        self.tasks
            .read()
            .map(|g| g.values().filter(|t| t.state == TaskState::Pending).count())
            .unwrap_or(0)
    }
}

// ── Events ────────────────────────────────────────────────

macro_rules! task_event {
    ($name:ident, $event_type:expr) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            pub task_id: TaskId,
        }

        impl ai_os_core::events::Event for $name {
            fn event_type(&self) -> &'static str {
                $event_type
            }
        }
    };
}

task_event!(TaskCreated, "runtime.task_created");
task_event!(TaskStarted, "runtime.task_started");
task_event!(TaskCompleted, "runtime.task_completed");
task_event!(TaskFailed, "runtime.task_failed");
task_event!(TaskCancelled, "runtime.task_cancelled");

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get() {
        let mgr = DefaultTaskManager::new();
        let task = mgr.create_task(None, Priority::NORMAL, HashMap::new());
        assert_eq!(task.state, TaskState::Pending);
        let fetched = mgr.get_task(&task.id);
        assert!(fetched.is_some());
    }

    #[test]
    fn state_transitions() {
        let mgr = DefaultTaskManager::new();
        let task = mgr.create_task(None, Priority::NORMAL, HashMap::new());

        mgr.update_state(&task.id, TaskState::Running).unwrap();
        assert_eq!(mgr.get_task(&task.id).unwrap().state, TaskState::Running);

        mgr.update_state(&task.id, TaskState::Completed).unwrap();
        assert_eq!(mgr.get_task(&task.id).unwrap().state, TaskState::Completed);
    }

    #[test]
    fn update_missing_fails() {
        let mgr = DefaultTaskManager::new();
        let id = TaskId::new();
        assert!(mgr.update_state(&id, TaskState::Running).is_err());
    }

    #[test]
    fn list_and_count() {
        let mgr = DefaultTaskManager::new();
        mgr.create_task(None, Priority::LOW, HashMap::new());
        mgr.create_task(None, Priority::HIGH, HashMap::new());
        assert_eq!(mgr.task_count(), 2);
        assert_eq!(mgr.pending_count(), 2);
    }

    #[test]
    fn tasks_by_session() {
        let mgr = DefaultTaskManager::new();
        let sid = SessionId::new();
        mgr.create_task(Some(sid.clone()), Priority::NORMAL, HashMap::new());
        mgr.create_task(None, Priority::NORMAL, HashMap::new());

        let session_tasks = mgr.list_tasks_by_session(&sid);
        assert_eq!(session_tasks.len(), 1);
    }

    #[test]
    fn priority_ordering() {
        assert!(Priority::CRITICAL < Priority::HIGH);
        assert!(Priority::HIGH < Priority::NORMAL);
        assert!(Priority::NORMAL < Priority::LOW);
    }
}
