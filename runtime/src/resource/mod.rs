//! # Resource Manager
//!
//! Tracks resource usage per task and enforces limits.
//!
//! ## Design
//!
//! * **Per-task tracking** — each task's CPU and memory usage
//!   is stored separately.
//! * **Aggregate** — `total_usage()` sums across all tasks.
//! * **Limit enforcement** — `check_limits()` returns an error
//!   if adding `additional` usage would exceed configured caps.
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;
use crate::task::TaskId;

// ── Resource usage ───────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

impl ResourceUsage {
    pub fn saturating_add(&self, other: &ResourceUsage) -> Self {
        Self {
            cpu_percent: (self.cpu_percent + other.cpu_percent).min(100.0),
            memory_bytes: self.memory_bytes.saturating_add(other.memory_bytes),
        }
    }
}

// ── Resource limits ──────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_cpu_percent: f64,
    pub max_memory_bytes: u64,
    pub max_tasks: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_percent: 100.0,
            max_memory_bytes: 1024 * 1024 * 1024, // 1 GB
            max_tasks: 100,
        }
    }
}

// ── Resource manager trait ───────────────────────────────

/// Tracks and limits resource consumption.
pub trait ResourceManager: Debug + Send + Sync {
    fn track(&self, task_id: &TaskId, usage: ResourceUsage) -> Result<(), RuntimeError>;
    fn usage(&self, task_id: &TaskId) -> Option<ResourceUsage>;
    fn total_usage(&self) -> ResourceUsage;
    fn limits(&self) -> ResourceLimits;
    fn set_limits(&self, limits: ResourceLimits) -> Result<(), RuntimeError>;
    fn check_limits(&self, additional: &ResourceUsage) -> Result<(), RuntimeError>;
    fn reset(&self, task_id: &TaskId) -> Result<(), RuntimeError>;
}

// ── Implementation ───────────────────────────────────────

#[derive(Debug)]
pub struct DefaultResourceManager {
    usage: RwLock<HashMap<TaskId, ResourceUsage>>,
    limits: RwLock<ResourceLimits>,
}

impl DefaultResourceManager {
    pub fn new() -> Self {
        Self {
            usage: RwLock::new(HashMap::new()),
            limits: RwLock::new(ResourceLimits::default()),
        }
    }
}

impl Default for DefaultResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceManager for DefaultResourceManager {
    fn track(&self, task_id: &TaskId, usage: ResourceUsage) -> Result<(), RuntimeError> {
        let mut guard = self.usage.write().map_err(|_| {
            RuntimeError::Resource("lock poisoned".into())
        })?;
        guard.insert(task_id.clone(), usage);
        Ok(())
    }

    fn usage(&self, task_id: &TaskId) -> Option<ResourceUsage> {
        self.usage.read().ok()?.get(task_id).copied()
    }

    fn total_usage(&self) -> ResourceUsage {
        let guard = match self.usage.read() {
            Ok(g) => g,
            Err(_) => return ResourceUsage::default(),
        };
        let mut total = ResourceUsage::default();
        for u in guard.values() {
            total = total.saturating_add(u);
        }
        total
    }

    fn limits(&self) -> ResourceLimits {
        self.limits.read().map(|g| *g).unwrap_or_default()
    }

    fn set_limits(&self, limits: ResourceLimits) -> Result<(), RuntimeError> {
        let mut guard = self.limits.write().map_err(|_| {
            RuntimeError::Resource("lock poisoned".into())
        })?;
        *guard = limits;
        Ok(())
    }

    fn check_limits(&self, additional: &ResourceUsage) -> Result<(), RuntimeError> {
        let limits = self.limits();
        let current = self.total_usage();
        let projected = current.saturating_add(additional);

        if projected.cpu_percent > limits.max_cpu_percent {
            return Err(RuntimeError::Resource(format!(
                "CPU limit: {:.1}% > {:.1}%",
                projected.cpu_percent, limits.max_cpu_percent
            )));
        }
        if projected.memory_bytes > limits.max_memory_bytes {
            return Err(RuntimeError::Resource(format!(
                "memory limit: {} > {}",
                projected.memory_bytes, limits.max_memory_bytes
            )));
        }
        Ok(())
    }

    fn reset(&self, task_id: &TaskId) -> Result<(), RuntimeError> {
        let mut guard = self.usage.write().map_err(|_| {
            RuntimeError::Resource("lock poisoned".into())
        })?;
        guard.remove(task_id).ok_or_else(|| {
            RuntimeError::Resource(format!("task {} not tracked", task_id))
        })?;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_and_read() {
        let rm = DefaultResourceManager::new();
        let id = TaskId::new();
        let usage = ResourceUsage {
            cpu_percent: 10.0,
            memory_bytes: 1024,
        };
        rm.track(&id, usage).unwrap();
        let read = rm.usage(&id).unwrap();
        assert!((read.cpu_percent - 10.0).abs() < 1e-6);
        assert_eq!(read.memory_bytes, 1024);
    }

    #[test]
    fn total_usage_sums() {
        let rm = DefaultResourceManager::new();
        rm.track(&TaskId::new(), ResourceUsage { cpu_percent: 10.0, memory_bytes: 512 }).unwrap();
        rm.track(&TaskId::new(), ResourceUsage { cpu_percent: 20.0, memory_bytes: 1024 }).unwrap();
        let total = rm.total_usage();
        assert!((total.cpu_percent - 30.0).abs() < 1e-6);
        assert_eq!(total.memory_bytes, 1536);
    }

    #[test]
    fn check_limits_passes() {
        let rm = DefaultResourceManager::new();
        rm.set_limits(ResourceLimits {
            max_cpu_percent: 50.0,
            max_memory_bytes: 10000,
            max_tasks: 10,
        })
        .unwrap();
        assert!(rm
            .check_limits(&ResourceUsage {
                cpu_percent: 10.0,
                memory_bytes: 500,
            })
            .is_ok());
    }

    #[test]
    fn check_limits_fails_on_cpu() {
        let rm = DefaultResourceManager::new();
        rm.set_limits(ResourceLimits {
            max_cpu_percent: 50.0,
            max_memory_bytes: 10000,
            max_tasks: 10,
        })
        .unwrap();
        // First task uses 45%
        rm.track(
            &TaskId::new(),
            ResourceUsage {
                cpu_percent: 45.0,
                memory_bytes: 100,
            },
        )
        .unwrap();
        // Additional 10% would exceed 50% limit
        assert!(rm
            .check_limits(&ResourceUsage {
                cpu_percent: 10.0,
                memory_bytes: 100,
            })
            .is_err());
    }

    #[test]
    fn reset_removes_task() {
        let rm = DefaultResourceManager::new();
        let id = TaskId::new();
        rm.track(&id, ResourceUsage::default()).unwrap();
        rm.reset(&id).unwrap();
        assert!(rm.usage(&id).is_none());
    }

    #[test]
    fn saturating_add_clamps_cpu() {
        let a = ResourceUsage { cpu_percent: 60.0, memory_bytes: 100 };
        let b = ResourceUsage { cpu_percent: 50.0, memory_bytes: 200 };
        let c = a.saturating_add(&b);
        assert!((c.cpu_percent - 100.0).abs() < 1e-6);
        assert_eq!(c.memory_bytes, 300);
    }
}
