//! # Supervisor
//!
//! Fault-tolerant supervisor that monitors tasks and applies
//! restart policies.
//!
//! ## Restart policies
//!
//! | Policy | Behaviour |
//! |---|---|
//! | `Never` | Do not restart. |
//! | `OnFailure` | Restart up to `max_retries` times on failure. |
//! | `Always` | Restart up to `max_retries` times regardless of outcome. |
//!
//! ## Design
//!
//! * **Event-driven** — the supervisor listens for
//!   `TaskFailed` events on the Core EventBus.
//! * **Per-task policy** — each supervised task has its own
//!   restart policy and retry count.
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;
use crate::task::TaskId;

// ── Restart policy ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RestartPolicy {
    Never,
    OnFailure { max_retries: u32 },
    Always { max_retries: u32 },
}

impl RestartPolicy {
    pub fn should_restart(&self, current_retries: u32, was_failure: bool) -> bool {
        match self {
            RestartPolicy::Never => false,
            RestartPolicy::OnFailure { max_retries } => {
                was_failure && current_retries <= *max_retries
            }
            RestartPolicy::Always { max_retries } => current_retries <= *max_retries,
        }
    }
}

// ── Supervision status ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionStatus {
    pub task_id: TaskId,
    pub policy: RestartPolicy,
    pub retries: u32,
    pub last_failure: Option<DateTime<Utc>>,
    pub last_failure_reason: Option<String>,
    pub is_active: bool,
}

// ── Supervisor trait ─────────────────────────────────────

/// Monitors and restarts tasks based on policy.
pub trait Supervisor: Debug + Send + Sync {
    fn supervise(&self, task_id: TaskId, policy: RestartPolicy) -> Result<(), RuntimeError>;
    fn cancel_supervision(&self, task_id: &TaskId) -> Result<(), RuntimeError>;
    fn status(&self, task_id: &TaskId) -> Option<SupervisionStatus>;
    fn list_supervised(&self) -> Vec<SupervisionStatus>;
    fn record_failure(&self, task_id: &TaskId, reason: &str) -> Result<bool, RuntimeError>;
}

// ── Implementation ───────────────────────────────────────

#[derive(Debug)]
pub struct DefaultSupervisor {
    supervised: RwLock<HashMap<TaskId, SupervisionStatus>>,
}

impl DefaultSupervisor {
    pub fn new() -> Self {
        Self {
            supervised: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor for DefaultSupervisor {
    fn supervise(&self, task_id: TaskId, policy: RestartPolicy) -> Result<(), RuntimeError> {
        let mut guard = self.supervised.write().map_err(|_| {
            RuntimeError::Supervisor("lock poisoned".into())
        })?;
        let status = SupervisionStatus {
            task_id: task_id.clone(),
            policy,
            retries: 0,
            last_failure: None,
            last_failure_reason: None,
            is_active: true,
        };
        guard.insert(task_id, status);
        Ok(())
    }

    fn cancel_supervision(&self, task_id: &TaskId) -> Result<(), RuntimeError> {
        let mut guard = self.supervised.write().map_err(|_| {
            RuntimeError::Supervisor("lock poisoned".into())
        })?;
        guard.remove(task_id).ok_or_else(|| {
            RuntimeError::Supervisor(format!("task {} not supervised", task_id))
        })?;
        Ok(())
    }

    fn status(&self, task_id: &TaskId) -> Option<SupervisionStatus> {
        self.supervised.read().ok()?.get(task_id).cloned()
    }

    fn list_supervised(&self) -> Vec<SupervisionStatus> {
        self.supervised
            .read()
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }

    fn record_failure(&self, task_id: &TaskId, reason: &str) -> Result<bool, RuntimeError> {
        let mut guard = self.supervised.write().map_err(|_| {
            RuntimeError::Supervisor("lock poisoned".into())
        })?;
        let status = guard.get_mut(task_id).ok_or_else(|| {
            RuntimeError::Supervisor(format!("task {} not supervised", task_id))
        })?;

        status.retries += 1;
        status.last_failure = Some(Utc::now());
        status.last_failure_reason = Some(reason.to_string());

        Ok(status.policy.should_restart(status.retries, true))
    }
}

// ── Event ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TaskRestarting {
    pub task_id: TaskId,
    pub retry_count: u32,
}

impl ai_os_core::events::Event for TaskRestarting {
    fn event_type(&self) -> &'static str {
        "runtime.task_restarting"
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervise_and_status() {
        let sup = DefaultSupervisor::new();
        let id = TaskId::new();
        sup.supervise(id.clone(), RestartPolicy::Never).unwrap();
        let status = sup.status(&id);
        assert!(status.is_some());
        assert!(status.unwrap().is_active);
    }

    #[test]
    fn never_policy_does_not_restart() {
        let sup = DefaultSupervisor::new();
        let id = TaskId::new();
        sup.supervise(id.clone(), RestartPolicy::Never).unwrap();
        let should = sup.record_failure(&id, "error").unwrap();
        assert!(!should);
    }

    #[test]
    fn on_failure_restarts_up_to_max() {
        let sup = DefaultSupervisor::new();
        let id = TaskId::new();
        sup.supervise(
            id.clone(),
            RestartPolicy::OnFailure { max_retries: 3 },
        )
        .unwrap();

        // Three retries allowed
        assert!(sup.record_failure(&id, "err1").unwrap());
        assert!(sup.record_failure(&id, "err2").unwrap());
        assert!(sup.record_failure(&id, "err3").unwrap());

        // Fourth failure: max retries exhausted
        assert!(!sup.record_failure(&id, "err4").unwrap());
    }

    #[test]
    fn always_policy_restarts_even_on_success() {
        let policy = RestartPolicy::Always { max_retries: 2 };
        // "was_failure = false" — Always still restarts up to max
        assert!(policy.should_restart(1, false));
        assert!(policy.should_restart(2, false));
        assert!(!policy.should_restart(3, false));
    }

    #[test]
    fn cancel_supervision() {
        let sup = DefaultSupervisor::new();
        let id = TaskId::new();
        sup.supervise(id.clone(), RestartPolicy::Never).unwrap();
        sup.cancel_supervision(&id).unwrap();
        assert!(sup.status(&id).is_none());
    }

    #[test]
    fn cancel_missing_fails() {
        let sup = DefaultSupervisor::new();
        let id = TaskId::new();
        assert!(sup.cancel_supervision(&id).is_err());
    }

    #[test]
    fn list_supervised() {
        let sup = DefaultSupervisor::new();
        sup.supervise(TaskId::new(), RestartPolicy::Never).unwrap();
        sup.supervise(TaskId::new(), RestartPolicy::Never).unwrap();
        assert_eq!(sup.list_supervised().len(), 2);
    }
}
