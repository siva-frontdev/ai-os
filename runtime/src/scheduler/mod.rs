//! # Scheduler
//!
//! Priority-based task scheduler with an in-memory work queue.
//!
//! ## Design
//!
//! * **Binary heap** — tasks are ordered by priority (lower
//!   numeric value = higher priority) using a
//!   `std::collections::BinaryHeap`.
//! * **Batch enqueue** — tasks can be enqueued individually or
//!   in batches.
//! * **Stats** — the scheduler tracks enqueue/dequeue counts
//!   and queue depth.
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, RwLock};

use crate::error::RuntimeError;
use crate::task::{Priority, TaskHandle, TaskId};

// ── Priority-ordered wrapper ─────────────────────────────

#[derive(Debug, Clone)]
struct ScheduledTask {
    handle: TaskHandle,
}

impl Eq for ScheduledTask {}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.handle.task_id == other.handle.task_id
    }
}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    // BinaryHeap is a max‑heap.  We reverse so that:
    //   • lower priority number (higher urgency) pops first;
    //   • equal priority → earlier created_at pops first (FIFO).
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .handle
            .priority
            .cmp(&self.handle.priority)
            .then_with(|| other.handle.created_at.cmp(&self.handle.created_at))
    }
}

// ── Scheduler stats ──────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    pub enqueued: u64,
    pub dequeued: u64,
    pub queue_depth: usize,
}

// ── Scheduler trait ──────────────────────────────────────

/// Task scheduling interface.
pub trait Scheduler: Debug + Send + Sync {
    fn enqueue(&self, handle: TaskHandle) -> Result<(), RuntimeError>;
    fn dequeue(&self) -> Option<TaskHandle>;
    fn peek(&self) -> Option<TaskHandle>;
    fn remove(&self, task_id: &TaskId) -> Result<(), RuntimeError>;
    fn stats(&self) -> SchedulerStats;
    fn queue_depth(&self) -> usize;
}

// ── Implementation (priority heap) ───────────────────────

#[derive(Debug)]
pub struct PriorityScheduler {
    queue: RwLock<BinaryHeap<ScheduledTask>>,
    enqueued: AtomicU64,
    dequeued: AtomicU64,
}

impl PriorityScheduler {
    pub fn new() -> Self {
        Self {
            queue: RwLock::new(BinaryHeap::new()),
            enqueued: AtomicU64::new(0),
            dequeued: AtomicU64::new(0),
        }
    }
}

impl Default for PriorityScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for PriorityScheduler {
    fn enqueue(&self, handle: TaskHandle) -> Result<(), RuntimeError> {
        let mut guard = self.queue.write().map_err(|_| {
            RuntimeError::Scheduler("lock poisoned".into())
        })?;
        guard.push(ScheduledTask { handle });
        self.enqueued.fetch_add(1, AtomicOrdering::Relaxed);
        Ok(())
    }

    fn dequeue(&self) -> Option<TaskHandle> {
        let mut guard = self.queue.write().ok()?;
        let task = guard.pop()?;
        self.dequeued.fetch_add(1, AtomicOrdering::Relaxed);
        Some(task.handle)
    }

    fn peek(&self) -> Option<TaskHandle> {
        let guard = self.queue.read().ok()?;
        guard.peek().map(|t| t.handle.clone())
    }

    fn remove(&self, task_id: &TaskId) -> Result<(), RuntimeError> {
        let mut guard = self.queue.write().map_err(|_| {
            RuntimeError::Scheduler("lock poisoned".into())
        })?;
        let before = guard.len();
        guard.retain(|t| t.handle.task_id != *task_id);
        if guard.len() == before {
            return Err(RuntimeError::Scheduler(format!(
                "task {} not in queue",
                task_id
            )));
        }
        Ok(())
    }

    fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            enqueued: self.enqueued.load(AtomicOrdering::Relaxed),
            dequeued: self.dequeued.load(AtomicOrdering::Relaxed),
            queue_depth: self.queue_depth(),
        }
    }

    fn queue_depth(&self) -> usize {
        self.queue.read().map(|g| g.len()).unwrap_or(0)
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_handle(priority: Priority, id: &str) -> TaskHandle {
        TaskHandle {
            task_id: TaskId::new(),
            priority,
            session_id: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn fifo_order_for_same_priority() {
        let sched = PriorityScheduler::new();
        let h1 = make_handle(Priority::NORMAL, "a");
        let h2 = make_handle(Priority::NORMAL, "b");
        sched.enqueue(h1.clone()).unwrap();
        sched.enqueue(h2.clone()).unwrap();

        assert_eq!(sched.dequeue().unwrap().task_id, h1.task_id);
        assert_eq!(sched.dequeue().unwrap().task_id, h2.task_id);
    }

    #[test]
    fn high_priority_comes_first() {
        let sched = PriorityScheduler::new();
        let low = make_handle(Priority::LOW, "low");
        let high = make_handle(Priority::HIGH, "high");
        sched.enqueue(low).unwrap();
        sched.enqueue(high.clone()).unwrap();

        assert_eq!(sched.dequeue().unwrap().task_id, high.task_id);
    }

    #[test]
    fn dequeue_empty_returns_none() {
        let sched = PriorityScheduler::new();
        assert!(sched.dequeue().is_none());
    }

    #[test]
    fn peek_does_not_remove() {
        let sched = PriorityScheduler::new();
        let h = make_handle(Priority::NORMAL, "x");
        sched.enqueue(h.clone()).unwrap();
        assert_eq!(sched.peek().unwrap().task_id, h.task_id);
        assert_eq!(sched.queue_depth(), 1);
    }

    #[test]
    fn remove_existing() {
        let sched = PriorityScheduler::new();
        let h = make_handle(Priority::NORMAL, "x");
        let id = h.task_id.clone();
        sched.enqueue(h).unwrap();
        sched.remove(&id).unwrap();
        assert_eq!(sched.queue_depth(), 0);
    }

    #[test]
    fn remove_missing_fails() {
        let sched = PriorityScheduler::new();
        let id = TaskId::new();
        assert!(sched.remove(&id).is_err());
    }

    #[test]
    fn stats_track_counts() {
        let sched = PriorityScheduler::new();
        sched.enqueue(make_handle(Priority::NORMAL, "a")).unwrap();
        sched.enqueue(make_handle(Priority::NORMAL, "b")).unwrap();
        sched.dequeue();

        let stats = sched.stats();
        assert_eq!(stats.enqueued, 2);
        assert_eq!(stats.dequeued, 1);
        assert_eq!(stats.queue_depth, 1);
    }
}
